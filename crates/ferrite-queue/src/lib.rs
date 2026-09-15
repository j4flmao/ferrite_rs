//! # Ferrite Queue
//!
//! Job queue abstraction for Ferrite. Provides a pluggable dispatcher
//! interface with an in-memory default (for dev/tests) and a Redis Streams
//! adapter (for production).
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ferrite_queue::{Job, JobHandler, MemoryQueue, Queue, submit_job_handler};
//! use serde::{Deserialize, Serialize};
//! use async_trait::async_trait;
//! use ferrite_macros::{module, injectable};
//! use std::sync::Arc;
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! pub struct SendEmail {
//!     pub to: String,
//!     pub template: String,
//! }
//!
//! impl Job for SendEmail {
//!     const NAME: &'static str = "email:send";
//! }
//!
//! #[injectable]
//! pub struct SendEmailHandler;
//!
//! #[async_trait]
//! impl JobHandler<SendEmail> for SendEmailHandler {
//!     async fn handle(&self, job: SendEmail) -> Result<(), ferrite_queue::JobError> {
//!         println!("Sending {} template to {}", job.template, job.to);
//!         Ok(())
//!     }
//! }
//!
//! submit_job_handler!(SendEmailHandler, SendEmail);
//!
//! #[module(providers = [SendEmailHandler])]
//! pub struct AppModule;
//! ```

use std::any::{Any, TypeId};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use dashmap::DashMap;
use ferrite_config::ConfigService;
use fr_core::{ModuleDescriptor, Scope};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum JobError {
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("dispatcher error: {0}")]
    Dispatch(String),
    #[error("job handler failed: {0}")]
    Handler(String),
    #[error("max attempts reached")]
    MaxAttempts,
    #[error("redis error: {0}")]
    Redis(String),
}

// ---------------------------------------------------------------------------
// Job trait & envelope
// ---------------------------------------------------------------------------

pub trait Job: Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static {
    const NAME: &'static str;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEnvelope {
    pub id: String,
    pub name: String,
    pub payload: Value,
    pub attempts: u8,
    pub max_attempts: u8,
    pub created_at: u64,
    pub queue: String,
}

impl JobEnvelope {
    pub fn new<J: Job>(job: &J, queue: impl Into<String>) -> Result<Self, JobError> {
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            name: J::NAME.to_string(),
            payload: serde_json::to_value(job)?,
            attempts: 0,
            max_attempts: 5,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            queue: queue.into(),
        })
    }
}

// ---------------------------------------------------------------------------
// Job handler trait + inventory registry
// ---------------------------------------------------------------------------

#[async_trait]
pub trait JobHandler<J: Job>: Send + Sync + 'static {
    async fn handle(&self, job: J) -> Result<(), JobError>;
}

#[derive(Copy, Clone)]
pub struct JobHandlerDescriptor {
    pub job_type: TypeId,
    pub job_name: &'static str,
    pub handler_type: TypeId,
    /// Downcast the handler from DI (AnyArc) and process an envelope.
    pub process: JobProcessFn,
}

type JobProcessFn = fn(
    handler_any: fr_core::AnyArc,
    envelope: JobEnvelope,
) -> std::pin::Pin<Box<dyn Future<Output = Result<(), JobError>> + Send>>;

inventory::collect!(JobHandlerDescriptor);

/// Generate a handler descriptor and submit it into the inventory registry.
#[macro_export]
macro_rules! submit_job_handler {
    ($handler_ty:ty, $job_ty:ty) => {
        const _: () = {
            use async_trait::async_trait;
            use fr_core::Injectable;
            use $crate::{Job, JobEnvelope, JobError, JobHandler, JobHandlerDescriptor};

            fn process(
                handler_any: fr_core::AnyArc,
                envelope: JobEnvelope,
            ) -> std::pin::Pin<Box<dyn Future<Output = Result<(), JobError>> + Send>> {
                Box::pin(async move {
                    let handler: std::sync::Arc<$handler_ty> =
                        handler_any.downcast::<$handler_ty>().map_err(|_| {
                            JobError::Dispatch(format!(
                                "DI container could not provide handler {}",
                                std::any::type_name::<$handler_ty>()
                            ))
                        })?;
                    let job: $job_ty = serde_json::from_value(envelope.payload)?;
                    JobHandler::<$job_ty>::handle(handler.as_ref(), job).await
                })
            }

            ::inventory::submit! {
                JobHandlerDescriptor {
                    job_type: std::any::TypeId::of::<$job_ty>(),
                    job_name: <$job_ty as Job>::NAME,
                    handler_type: std::any::TypeId::of::<$handler_ty>(),
                    process,
                }
            }
        };
    };
}

// ---------------------------------------------------------------------------
// Queue dispatcher trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait Queue: Send + Sync + 'static {
    async fn enqueue(&self, envelope: JobEnvelope) -> Result<String, JobError>;
    async fn dequeue(&self, queue: &str, limit: usize) -> Result<Vec<JobEnvelope>, JobError>;
    async fn ack(&self, queue: &str, id: &str) -> Result<(), JobError>;
    async fn retry(&self, envelope: JobEnvelope) -> Result<(), JobError>;
    async fn fail(&self, envelope: JobEnvelope, reason: &str) -> Result<(), JobError>;
    fn queue_name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Memory queue (default/dev/test)
// ---------------------------------------------------------------------------

struct MemoryInner {
    queues: DashMap<String, VecDeque<JobEnvelope>>,
    /// Jobs that failed and are waiting for manual inspection (optional DLQ).
    dead_letter: DashMap<String, Vec<(String, String)>>,
}

pub struct MemoryQueue {
    inner: Arc<MemoryInner>,
    default_queue: String,
    _anchor: u8,
}

impl MemoryQueue {
    pub fn new(config: Arc<ConfigService>) -> Self {
        let default_queue = config.get_or_parse("QUEUE_NAME", "default".to_string());
        Self {
            inner: Arc::new(MemoryInner {
                queues: DashMap::new(),
                dead_letter: DashMap::new(),
            }),
            default_queue,
            _anchor: 0,
        }
    }
}

impl Default for MemoryQueue {
    fn default() -> Self {
        // Use ConfigService-less default for unit tests
        Self {
            inner: Arc::new(MemoryInner {
                queues: DashMap::new(),
                dead_letter: DashMap::new(),
            }),
            default_queue: "default".to_string(),
            _anchor: 0,
        }
    }
}

inventory::submit! {
    fr_core::ProviderEntry::new_static::<MemoryQueue>(
        "MemoryQueue",
        Scope::Singleton,
        || vec![TypeId::of::<ConfigService>()],
        |container| -> Arc<dyn Any + Send + Sync> {
            let cfg = container.get::<ConfigService>();
            Arc::new(MemoryQueue::new(cfg)) as Arc<dyn Any + Send + Sync>
        },
    )
}

#[async_trait]
impl Queue for MemoryQueue {
    async fn enqueue(&self, envelope: JobEnvelope) -> Result<String, JobError> {
        let id = envelope.id.clone();
        let q = envelope.queue.clone();
        self.inner.queues.entry(q).or_default().push_back(envelope);
        Ok(id)
    }

    async fn dequeue(&self, queue: &str, limit: usize) -> Result<Vec<JobEnvelope>, JobError> {
        let limit = limit.max(1);
        let mut out = Vec::with_capacity(limit);
        if let Some(mut q) = self.inner.queues.get_mut(queue) {
            while out.len() < limit {
                if let Some(mut env) = q.pop_front() {
                    env.attempts += 1;
                    // Re-insert so we keep a copy for retry handling until acked.
                    // Memory-only: don't keep pending set, just return directly.
                    out.push(env);
                } else {
                    break;
                }
            }
        }
        Ok(out)
    }

    async fn ack(&self, _queue: &str, _id: &str) -> Result<(), JobError> {
        // Memory queue is fire-from-front-dequeue: nothing to do on ack.
        Ok(())
    }

    async fn retry(&self, envelope: JobEnvelope) -> Result<(), JobError> {
        if envelope.attempts >= envelope.max_attempts {
            self.fail(envelope, "max_attempts").await?;
            return Err(JobError::MaxAttempts);
        }
        let q = envelope.queue.clone();
        self.inner.queues.entry(q).or_default().push_back(envelope);
        Ok(())
    }

    async fn fail(&self, envelope: JobEnvelope, reason: &str) -> Result<(), JobError> {
        self.inner
            .dead_letter
            .entry(envelope.queue.clone())
            .or_default()
            .push((envelope.id, reason.to_string()));
        Ok(())
    }

    fn queue_name(&self) -> &str {
        &self.default_queue
    }
}

// ---------------------------------------------------------------------------
// Redis Streams adapter (production)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct RedisQueue {
    redis_url: String,
    stream_name: String,
    group_name: String,
    consumer_name: String,
    client: Arc<std::sync::Mutex<Option<redis::Client>>>,
    conn: Arc<tokio::sync::Mutex<Option<redis::aio::MultiplexedConnection>>>,
}

impl RedisQueue {
    pub fn new(config: Arc<ConfigService>) -> Self {
        let redis_url = config
            .get("REDIS_URL")
            .unwrap_or_else(|| "redis://127.0.0.1:6379".to_string());
        let stream_name = config.get_or_parse("QUEUE_STREAM", "ferrite:jobs".to_string());
        let group_name = config.get_or_parse("QUEUE_GROUP", "workers".to_string());
        let consumer_name =
            config.get_or_parse("QUEUE_CONSUMER", format!("worker-{}", Uuid::new_v4()));
        Self {
            redis_url,
            stream_name,
            group_name,
            consumer_name,
            client: Arc::new(std::sync::Mutex::new(None)),
            conn: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    async fn ensure_conn(&self) -> Result<(), JobError> {
        let mut conn_lock = self.conn.lock().await;
        if conn_lock.is_none() {
            let client = {
                let mut client_lock = self.client.lock().unwrap();
                if client_lock.is_none() {
                    *client_lock = Some(
                        redis::Client::open(self.redis_url.as_str())
                            .map_err(|e| JobError::Redis(e.to_string()))?,
                    );
                }
                client_lock.as_ref().unwrap().clone()
            };
            let c = client
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| JobError::Redis(e.to_string()))?;
            *conn_lock = Some(c);
            // Ensure the consumer group exists on first connect.
            let c2 = conn_lock.as_mut().unwrap();
            let _: Result<redis::Value, _> = redis::cmd("XGROUP")
                .arg("CREATE")
                .arg(&self.stream_name)
                .arg(&self.group_name)
                .arg("0")
                .arg("MKSTREAM")
                .query_async(c2)
                .await;
        }
        Ok(())
    }
}

#[async_trait]
impl Queue for RedisQueue {
    async fn enqueue(&self, envelope: JobEnvelope) -> Result<String, JobError> {
        self.ensure_conn().await?;
        let mut lock = self.conn.lock().await;
        let conn = lock.as_mut().unwrap();
        let payload = serde_json::to_string(&envelope)?;
        let id: String = redis::cmd("XADD")
            .arg(&self.stream_name)
            .arg("*")
            .arg("payload")
            .arg(payload.as_str())
            .query_async(conn)
            .await
            .map_err(|e| JobError::Redis(e.to_string()))?;
        Ok(id)
    }

    async fn dequeue(&self, _queue: &str, limit: usize) -> Result<Vec<JobEnvelope>, JobError> {
        use redis::FromRedisValue;
        self.ensure_conn().await?;
        let mut lock = self.conn.lock().await;
        let conn = lock.as_mut().unwrap();
        let limit = limit.max(1);
        let read = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(&self.group_name)
            .arg(&self.consumer_name)
            .arg("COUNT")
            .arg(limit)
            .arg("BLOCK")
            .arg(1000u64)
            .arg("STREAMS")
            .arg(&self.stream_name)
            .arg(">")
            .query_async::<redis::Value>(conn)
            .await
            .map_err(|e| JobError::Redis(e.to_string()))?;

        let mut out = Vec::new();
        if let redis::Value::Array(streams) = read {
            for stream_val in streams {
                if let redis::Value::Array(elements) = stream_val {
                    if let Some(redis::Value::Array(entries)) = elements.into_iter().nth(1) {
                        for entry in entries {
                            if let redis::Value::Array(pair) = entry {
                                if pair.len() >= 2 {
                                    if let redis::Value::Array(fields) = &pair[1] {
                                        let mut i = 0;
                                        while i + 1 < fields.len() {
                                            if let (redis::Value::BulkString(k), v) =
                                                (&fields[i], &fields[i + 1])
                                            {
                                                if k == b"payload" {
                                                    if let Ok(s) = String::from_redis_value(v) {
                                                        if let Ok(env) =
                                                            serde_json::from_str::<JobEnvelope>(&s)
                                                        {
                                                            out.push(env);
                                                        }
                                                    }
                                                }
                                            }
                                            i += 2;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    async fn ack(&self, _queue: &str, id: &str) -> Result<(), JobError> {
        self.ensure_conn().await?;
        let mut lock = self.conn.lock().await;
        let conn = lock.as_mut().unwrap();
        let _: redis::Value = redis::cmd("XACK")
            .arg(&self.stream_name)
            .arg(&self.group_name)
            .arg(id)
            .query_async(conn)
            .await
            .map_err(|e| JobError::Redis(e.to_string()))?;
        Ok(())
    }

    async fn retry(&self, envelope: JobEnvelope) -> Result<(), JobError> {
        if envelope.attempts >= envelope.max_attempts {
            self.fail(envelope, "max_attempts").await?;
            return Err(JobError::MaxAttempts);
        }
        self.enqueue(envelope).await?;
        Ok(())
    }

    async fn fail(&self, envelope: JobEnvelope, _reason: &str) -> Result<(), JobError> {
        // Store to a dead-letter hash for manual inspection.
        use redis::AsyncCommands;
        self.ensure_conn().await?;
        let mut lock = self.conn.lock().await;
        let conn = lock.as_mut().unwrap();
        let dl_key = format!("{}:dl", self.stream_name);
        let val = serde_json::to_string(&envelope)?;
        let _: redis::Value = conn
            .hset_nx(&dl_key, envelope.id, val)
            .await
            .map_err(|e| JobError::Redis(e.to_string()))?;
        Ok(())
    }

    fn queue_name(&self) -> &str {
        &self.stream_name
    }
}

// ---------------------------------------------------------------------------
// Dispatcher helper: dispatch typed Job -> Queue::enqueue
// ---------------------------------------------------------------------------

pub struct DispatcherService {
    queue: Arc<dyn Queue>,
    default_queue: String,
}

impl DispatcherService {
    pub fn new<Q: Queue + 'static>(queue: Arc<Q>, default_queue: impl Into<String>) -> Self {
        Self {
            queue: queue as Arc<dyn Queue>,
            default_queue: default_queue.into(),
        }
    }

    pub async fn dispatch<J: Job>(&self, job: &J) -> Result<String, JobError> {
        self.dispatch_to::<J>(job, &self.default_queue.clone())
            .await
    }

    pub async fn dispatch_to<J: Job>(&self, job: &J, queue: &str) -> Result<String, JobError> {
        let envelope = JobEnvelope::new(job, queue)?;
        self.queue.enqueue(envelope).await
    }
}

impl fr_core::Injectable for DispatcherService {
    fn __provider_entry() -> fr_core::ProviderEntry {
        use fr_core::ProviderEntry;
        ProviderEntry::new_static::<DispatcherService>(
            "DispatcherService",
            Scope::Singleton,
            || vec![TypeId::of::<MemoryQueue>(), TypeId::of::<ConfigService>()],
            |container| -> Arc<dyn Any + Send + Sync> {
                let q = container.get::<MemoryQueue>();
                let default_queue = q.queue_name().to_string();
                Arc::new(DispatcherService::new(q, default_queue)) as Arc<dyn Any + Send + Sync>
            },
        )
    }
}

inventory::submit! {
    fr_core::ProviderEntry::new_static::<DispatcherService>(
        "DispatcherService",
        Scope::Singleton,
        || vec![TypeId::of::<MemoryQueue>(), TypeId::of::<ConfigService>()],
        |container| -> Arc<dyn Any + Send + Sync> {
            let q = container.get::<MemoryQueue>();
            let default_queue = q.queue_name().to_string();
            Arc::new(DispatcherService::new(q, default_queue))
                as Arc<dyn Any + Send + Sync>
        },
    )
}

// ---------------------------------------------------------------------------
// Worker: runs queue loop with registered handler descriptors
// ---------------------------------------------------------------------------

pub struct WorkerService {
    queue: Arc<MemoryQueue>,
    container: fr_core::Container,
    poll_interval_ms: u64,
    batch_size: usize,
    _anchor: u8,
}

inventory::submit! {
    fr_core::ProviderEntry::new_static::<WorkerService>(
        "WorkerService",
        Scope::Singleton,
        || vec![
            TypeId::of::<MemoryQueue>(),
            TypeId::of::<ConfigService>(),
        ],
        |container| -> Arc<dyn Any + Send + Sync> {
            let q = container.get::<MemoryQueue>();
            let cfg = container.get::<ConfigService>();
            Arc::new(WorkerService::new(q, container.clone(), cfg))
                as Arc<dyn Any + Send + Sync>
        },
    )
}

impl WorkerService {
    pub fn new(
        queue: Arc<MemoryQueue>,
        container: fr_core::Container,
        config: Arc<ConfigService>,
    ) -> Self {
        let poll_interval_ms = config.get_or_parse("QUEUE_POLL_MS", 500u64);
        let batch_size = config.get_or_parse("QUEUE_BATCH", 4usize);
        Self {
            queue,
            container,
            poll_interval_ms,
            batch_size,
            _anchor: 0,
        }
    }

    /// Run the worker forever. Caller typically wraps in `tokio::spawn`.
    pub async fn run(&self) {
        loop {
            if let Err(err) = self.tick().await {
                eprintln!("[ferrite-queue] tick error: {err:?}");
            }
            tokio::time::sleep(Duration::from_millis(self.poll_interval_ms)).await;
        }
    }

    /// Drain a single batch from the queue and run registered handlers.
    /// Returns the number of jobs processed.
    pub async fn tick(&self) -> Result<usize, JobError> {
        let queue_name = self.queue.queue_name().to_string();
        let batch = self.queue.dequeue(&queue_name, self.batch_size).await?;
        let mut processed = 0usize;
        let handlers: HashMap<String, JobHandlerDescriptor> =
            inventory::iter::<JobHandlerDescriptor>()
                .map(|d| (d.job_name.to_string(), *d))
                .collect();
        for env in batch {
            let id = env.id.clone();
            if let Some(desc) = handlers.get(env.name.as_str()) {
                let handler_any = self.container.get_any(desc.handler_type);
                let fut = (desc.process)(handler_any, env.clone());
                match fut.await {
                    Ok(()) => {
                        self.queue.ack(&queue_name, &id).await?;
                    }
                    Err(e) => {
                        eprintln!(
                            "[ferrite-queue] job {} (name={}) failed: {e:?}",
                            id, env.name
                        );
                        let _ = self.queue.retry(env).await;
                    }
                }
                processed += 1;
            } else {
                let reason = format!("no handler for {}", env.name);
                eprintln!(
                    "[ferrite-queue] no handler registered for job name={} id={id}",
                    env.name
                );
                let _ = self.queue.fail(env, &reason).await;
            }
        }
        Ok(processed)
    }
}

// ---------------------------------------------------------------------------
// QueueModule
// ---------------------------------------------------------------------------

pub struct QueueModule;

impl QueueModule {
    pub fn for_root() -> QueueModuleImpl {
        QueueModuleImpl
    }
}

#[derive(Clone, Copy)]
pub struct QueueModuleImpl;

impl fr_core::Module for QueueModuleImpl {
    fn __module_descriptor() -> ModuleDescriptor {
        ModuleDescriptor {
            name: String::from("QueueModule"),
            providers: vec![
                TypeId::of::<MemoryQueue>(),
                TypeId::of::<DispatcherService>(),
                TypeId::of::<WorkerService>(),
            ],
            controllers: vec![],
            imports: vec![],
            exports: vec![
                TypeId::of::<DispatcherService>(),
                TypeId::of::<WorkerService>(),
            ],
            middleware: vec![],
            global: false,
        }
    }
}

impl fr_core::OnApplicationBootstrap for QueueModuleImpl {}

// ---------------------------------------------------------------------------
// Debug/display helpers
// ---------------------------------------------------------------------------

impl fmt::Debug for MemoryQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryQueue")
            .field("default_queue", &self.default_queue)
            .field("queues_len", &self.inner.queues.len())
            .finish()
    }
}

impl fmt::Debug for RedisQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedisQueue")
            .field("stream", &self.stream_name)
            .field("group", &self.group_name)
            .field("consumer", &self.consumer_name)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    struct TestJob {
        value: i32,
    }

    impl Job for TestJob {
        const NAME: &'static str = "test:job";
    }

    #[tokio::test]
    async fn memory_queue_enqueue_dequeue_ack_flow() {
        let q = MemoryQueue::default();
        let job = TestJob { value: 42 };
        let env = JobEnvelope::new(&job, "default").unwrap();
        let id = q.enqueue(env).await.unwrap();
        assert!(!id.is_empty());
        let batch = q.dequeue("default", 10).await.unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].attempts, 1);
        q.ack("default", &id).await.unwrap();
    }

    #[tokio::test]
    async fn memory_queue_retry_up_to_max_attempts() {
        let q = MemoryQueue::default();
        let mut env = JobEnvelope::new(&TestJob { value: 1 }, "q").unwrap();
        env.attempts = 5;
        env.max_attempts = 5;
        let err = q.retry(env).await.unwrap_err();
        assert!(matches!(err, JobError::MaxAttempts));
        let dl = q.inner.dead_letter.get("q").unwrap();
        assert_eq!(dl.len(), 1);
    }

    #[tokio::test]
    async fn dispatcher_round_trip_typed_job() {
        let q = Arc::new(MemoryQueue::default());
        let disp = DispatcherService::new(q.clone(), "default".to_string());
        disp.dispatch(&TestJob { value: 7 }).await.unwrap();
        let batch = q.dequeue("default", 1).await.unwrap();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].name, "test:job");
        let parsed: TestJob = serde_json::from_value(batch[0].payload.clone()).unwrap();
        assert_eq!(parsed.value, 7);
    }

    #[test]
    fn envelope_serializes_payload() {
        let j = TestJob { value: 9 };
        let env = JobEnvelope::new(&j, "q").unwrap();
        let back: TestJob = serde_json::from_value(env.payload).unwrap();
        assert_eq!(back, j);
    }
}
