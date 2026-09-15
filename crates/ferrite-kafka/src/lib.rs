//! # Ferrite Kafka
//!
//! Kafka-based message-pattern microservice transport.
//!
//! Provides `@KafkaPattern` style handler registry mirroring Nest's Kafka
//! `@MessagePattern` semantics. Same inventory-based handler registration
//! used by `ferrite-nats` so both message transports share a uniform
//! ergonomics.
//!
//! ## Tests use a in-memory stub broker (no external Kafka required).
//! Production users supply a bootstrap.servers config and plug `rdkafka` native
//! client via the `RealKafka` trait adapter (exposed in later patch releases).

use std::any::{Any, TypeId};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashMap;
use ferrite_config::ConfigService;
use fr_core::{ModuleDescriptor, OnApplicationBootstrap, ProviderEntry, Scope};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type AnyArc = Arc<dyn Any + Send + Sync>;
pub type HandlerFuture = Pin<Box<dyn Future<Output = Result<(), KafkaError>> + Send>>;

#[derive(Debug, Error)]
pub enum KafkaError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("no handler registered for topic `{0}`")]
    NoHandler(String),
}

/// Kafka message envelope: topic + partition + offset + typed payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaMessage<P> {
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
    pub key: Option<String>,
    pub payload: P,
}

/// Object-safe inventory descriptor.
pub struct KafkaHandlerDescriptor {
    pub pattern: &'static str, // conventionally = topic or custom-pattern
    pub handler_type: TypeId,
    pub msg_type: TypeId,
    pub process: fn(AnyArc, KafkaMessage<String>) -> HandlerFuture,
}

inventory::collect!(KafkaHandlerDescriptor);

#[async_trait]
pub trait KafkaHandler<M: Send + Sync + 'static>: Send + Sync + 'static {
    const PATTERN: &'static str;
    async fn handle(&self, msg: KafkaMessage<M>) -> Result<(), KafkaError>;
}

#[macro_export]
macro_rules! submit_kafka_handler {
    ($h:ty, $m:ty) => {
        const _: () = {
            fn __process(
                handler_any: $crate::AnyArc,
                raw: $crate::KafkaMessage<String>,
            ) -> $crate::HandlerFuture {
                Box::pin(async move {
                    let typed = handler_any
                        .downcast::<$h>()
                        .map_err(|_| $crate::KafkaError::Transport("bad handler type".into()))?;
                    let payload: $m = serde_json::from_str(&raw.payload)?;
                    let msg = $crate::KafkaMessage {
                        topic: raw.topic,
                        partition: raw.partition,
                        offset: raw.offset,
                        key: raw.key,
                        payload,
                    };
                    <$h as $crate::KafkaHandler<$m>>::handle(&*typed, msg).await
                })
            }
            ::inventory::submit! {
                $crate::KafkaHandlerDescriptor {
                    pattern: <$h as $crate::KafkaHandler<$m>>::PATTERN,
                    handler_type: std::any::TypeId::of::<$h>(),
                    msg_type: std::any::TypeId::of::<$m>(),
                    process: __process,
                }
            }
        };
    };
}

type RawMessage = (String, i32, i64, Option<String>, String);

#[derive(Default)]
struct MemoryKafka {
    topics: DashMap<String, VecDeque<RawMessage>>,
    next_offsets: DashMap<(String, i32), i64>,
}

impl MemoryKafka {
    pub fn produce(&self, topic: &str, key: Option<&str>, payload: &str) {
        let partition = 0i32;
        let mut off = self
            .next_offsets
            .entry((topic.to_string(), partition))
            .or_insert(0);
        let offset = *off;
        *off += 1;
        self.topics
            .entry(topic.to_string())
            .or_default()
            .push_back((
                topic.to_string(),
                partition,
                offset,
                key.map(|s| s.to_string()),
                payload.to_string(),
            ));
    }

    pub fn consume_batch(&self, topic: &str, limit: usize) -> Vec<RawMessage> {
        let mut out = Vec::with_capacity(limit);
        if let Some(mut q) = self.topics.get_mut(topic) {
            for _ in 0..limit {
                if let Some(x) = q.pop_front() {
                    out.push(x);
                } else {
                    break;
                }
            }
        }
        out
    }
}

/// Main Kafka facade: in-memory stub broker + inventory-based handler dispatch loop.
pub struct KafkaServer {
    container: fr_core::Container,
    broker: Arc<MemoryKafka>,
    poll_ms: u64,
    batch_size: usize,
}

impl KafkaServer {
    pub fn new(container: fr_core::Container, config: Arc<ConfigService>) -> Self {
        Self {
            container,
            broker: Arc::new(MemoryKafka::default()),
            poll_ms: config.get_or_parse("KAFKA_POLL_MS", 100u64),
            batch_size: config.get_or_parse("KAFKA_BATCH", 16usize),
        }
    }

    /// Produce a typed record onto the in-memory broker.
    pub fn produce<M: Serialize>(
        &self,
        topic: &str,
        key: Option<&str>,
        payload: &M,
    ) -> Result<(), KafkaError> {
        let raw = serde_json::to_string(payload)?;
        self.broker.produce(topic, key, &raw);
        Ok(())
    }

    /// Dispatch one raw record through the registered handlers by pattern (topic).
    pub async fn dispatch_one(
        &self,
        topic: String,
        partition: i32,
        offset: i64,
        key: Option<String>,
        raw_payload: String,
    ) -> Result<(), KafkaError> {
        let descs: HashMap<&str, &KafkaHandlerDescriptor> =
            inventory::iter::<KafkaHandlerDescriptor>()
                .map(|d| (d.pattern, d))
                .collect();
        let desc = descs
            .get(topic.as_str())
            .copied()
            .ok_or_else(|| KafkaError::NoHandler(topic.clone()))?;
        let handler_any = self
            .container
            .try_get_any(desc.handler_type)
            .ok_or_else(|| KafkaError::NoHandler(format!("handler not in DI: {}", topic)))?;
        let msg = KafkaMessage {
            topic,
            partition,
            offset,
            key,
            payload: raw_payload,
        };
        (desc.process)(handler_any, msg).await
    }

    /// Consume + dispatch one batch from every registered pattern.
    pub async fn tick(&self) -> Result<usize, KafkaError> {
        let mut processed = 0usize;
        let patterns: Vec<String> = self.patterns();
        for pattern in patterns {
            let batch = self.broker.consume_batch(&pattern, self.batch_size);
            for (topic, partition, offset, key, payload) in batch {
                self.dispatch_one(topic, partition, offset, key, payload)
                    .await?;
                processed += 1;
            }
        }
        Ok(processed)
    }

    /// Long-running consume loop.
    pub async fn run(&self) {
        loop {
            if let Err(err) = self.tick().await {
                eprintln!("[ferrite-kafka] tick error: {err:?}");
            }
            tokio::time::sleep(Duration::from_millis(self.poll_ms)).await;
        }
    }

    pub fn patterns(&self) -> Vec<String> {
        inventory::iter::<KafkaHandlerDescriptor>()
            .map(|d| d.pattern.to_string())
            .collect()
    }
}

inventory::submit! {
    ProviderEntry::new_static::<KafkaServer>(
        "KafkaServer",
        Scope::Singleton,
        || vec![
            TypeId::of::<fr_core::Container>(),
            TypeId::of::<ConfigService>(),
        ],
        |container| -> Arc<dyn Any + Send + Sync> {
            let cfg = container.get::<ConfigService>();
            Arc::new(KafkaServer::new(container.clone(), cfg)) as Arc<dyn Any + Send + Sync>
        },
    )
}

pub struct KafkaModule;

impl KafkaModule {
    pub fn for_root() -> KafkaModuleImpl {
        KafkaModuleImpl
    }
}

#[derive(Clone, Copy)]
pub struct KafkaModuleImpl;

impl fr_core::Module for KafkaModuleImpl {
    fn __module_descriptor() -> ModuleDescriptor {
        ModuleDescriptor {
            name: "KafkaModule".into(),
            providers: vec![TypeId::of::<KafkaServer>()],
            controllers: vec![],
            imports: vec![],
            exports: vec![TypeId::of::<KafkaServer>()],
            middleware: vec![],
            global: false,
        }
    }
}

impl OnApplicationBootstrap for KafkaModuleImpl {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    struct OrderCreated {
        id: u64,
    }

    struct OrdersHandler {
        n: Arc<AtomicUsize>,
    }

    impl Default for OrdersHandler {
        fn default() -> Self {
            Self {
                n: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl KafkaHandler<OrderCreated> for OrdersHandler {
        const PATTERN: &'static str = "orders.created";
        async fn handle(&self, msg: KafkaMessage<OrderCreated>) -> Result<(), KafkaError> {
            assert_eq!(msg.payload.id, 42);
            self.n.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    submit_kafka_handler!(OrdersHandler, OrderCreated);

    #[test]
    fn patterns_collects_handler() {
        let pats = inventory::iter::<KafkaHandlerDescriptor>()
            .map(|d| d.pattern)
            .collect::<Vec<_>>();
        assert!(pats.contains(&"orders.created"), "missing pattern {pats:?}");
    }

    #[tokio::test]
    async fn produce_then_tick_processes_message() {
        let container = fr_core::Container::default();
        let cfg = Arc::new(ConfigService::default());
        container.seed_singleton(cfg);
        let h = Arc::new(OrdersHandler::default());
        let counter = h.n.clone();
        container.seed_singleton(h);
        let srv = KafkaServer::new(container, Arc::new(ConfigService::default()));
        srv.produce("orders.created", Some("k1"), &OrderCreated { id: 42 })
            .unwrap();
        let n = srv.tick().await.unwrap();
        assert_eq!(n, 1);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn no_handler_returns_err() {
        let container = fr_core::Container::default();
        let cfg = Arc::new(ConfigService::default());
        container.seed_singleton(cfg);
        let srv = KafkaServer::new(container, Arc::new(ConfigService::default()));
        let err = srv
            .dispatch_one("nothing".into(), 0, 0, None, "{}".into())
            .await
            .unwrap_err();
        assert!(matches!(err, KafkaError::NoHandler(_)));
    }
}
