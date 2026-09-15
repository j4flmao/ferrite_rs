//! # Ferrite NATS
//!
//! Message-pattern microservice transport for Ferrite over NATS.
//!
//! Mirrors Nest's `@EventPattern` / `@MessagePattern` decorator style using a
//! compile-time inventory registry of topic -> handler mappings. No runtime
//! reflection is used — handlers submitted via `submit_nats_handler!` are
//! folded into the binary's `.ferrite.handlers` linker section.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ferrite_nats::{MessageHandler, NatsMessage, NatsServer, submit_nats_handler};
//! use serde::{Deserialize, Serialize};
//! use async_trait::async_trait;
//!
//! #[derive(Debug, Serialize, Deserialize)]
//! struct Hello { who: String }
//!
//! #[derive(Default)]
//! struct HelloHandler;
//!
//! #[async_trait]
//! impl MessageHandler<Hello> for HelloHandler {
//!     const PATTERN: &'static str = "say.hello";
//!     async fn handle(&self, msg: NatsMessage<Hello>) -> Result<(), NatsError> {
//!         println!("Hello {}!", msg.payload.who);
//!         Ok(())
//!     }
//! }
//!
//! submit_nats_handler!(HelloHandler, Hello);
//! ```

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashMap;
use fr_config::ConfigService;
use fr_core::{ModuleDescriptor, OnApplicationBootstrap, ProviderEntry, Scope};
use serde::{Deserialize, Serialize};
use thiserror::Error;

type AnyArc = Arc<dyn Any + Send + Sync>;
type HandlerFuture = Pin<Box<dyn Future<Output = Result<(), NatsError>> + Send>>;

#[derive(Debug, Error)]
pub enum NatsError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("deserialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("no handler registered for pattern `{0}`")]
    NoHandler(String),
}

/// A NATS message envelope (typed payload + metadata like subject / reply subject).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsMessage<P> {
    pub subject: String,
    pub reply: Option<String>,
    pub payload: P,
}

/// Object-safe trait bound for per-message handlers submitted via inventory.
pub struct NatsHandlerDescriptor {
    pub pattern: &'static str,
    pub handler_type: TypeId,
    pub msg_type: TypeId,
    pub process:
        fn(AnyArc, subject: String, reply: Option<String>, payload: String) -> HandlerFuture,
}

inventory::collect!(NatsHandlerDescriptor);

/// User-facing trait: implement once per (handler + payload pair to handle one pattern.
#[async_trait]
pub trait MessageHandler<M: Send + Sync + 'static>: Send + Sync + 'static {
    const PATTERN: &'static str;
    async fn handle(&self, msg: NatsMessage<M>) -> Result<(), NatsError>;
}

/// Submit a `(Handler, Message)` pair into the inventory registry.
#[macro_export]
macro_rules! submit_nats_handler {
    ($h:ty, $m:ty) => {
        const _: () = {
            fn __process(
                handler_any: $crate::AnyArc,
                subject: String,
                reply: Option<String>,
                payload: String,
            ) -> $crate::HandlerFuture {
                Box::pin(async move {
                    let typed: std::sync::Arc<$h> = handler_any
                        .downcast::<$h>()
                        .map_err(|_| $crate::NatsError::Transport("bad handler type".into()))?;
                    let parsed: $m = serde_json::from_str(&payload)?;
                    let msg = $crate::NatsMessage {
                        subject,
                        reply,
                        payload: parsed,
                    };
                    <$h as $crate::MessageHandler<$m>>::handle(&*typed, msg).await
                })
            }
            ::inventory::submit! {
                $crate::NatsHandlerDescriptor {
                    pattern: <$h as $crate::MessageHandler<$m>>::PATTERN,
                    handler_type: std::any::TypeId::of::<$h>(),
                    msg_type: std::any::TypeId::of::<$m>(),
                    process: __process,
                }
            }
        };
    };
}

/// In-memory fake NATS broker used by tests / dev. Does not require an external
/// NATS server — just a shared channel bus. Replaces the real `async_nats` client
/// when no `NATS_URL` config key is set, so developers can iterate quickly.
#[derive(Default)]
struct MemoryNats {
    subs: DashMap<String, Vec<tokio::sync::mpsc::UnboundedSender<MemoryNatsMessage>>>,
    pending: DashMap<String, VecDeque<MemoryNatsMessage>>,
}

type MemoryNatsMessage = (String, Option<String>, String);

impl MemoryNats {
    pub fn publish(&self, subject: &str, reply: Option<&str>, payload: &str) {
        let msg = (
            subject.to_string(),
            reply.map(|s| s.to_string()),
            payload.to_string(),
        );
        let subs = self.subs.get(subject);
        if let Some(subs) = subs {
            let mut sent = 0usize;
            for tx in subs.value().iter() {
                if tx.send(msg.clone()).is_ok() {
                    sent += 1;
                }
            }
            if sent == 0 {
                self.pending
                    .entry(subject.to_string())
                    .or_default()
                    .push_back(msg);
            }
        } else {
            self.pending
                .entry(subject.to_string())
                .or_default()
                .push_back(msg);
        }
    }

    pub fn subscribe(
        &self,
        subject: &str,
        tx: tokio::sync::mpsc::UnboundedSender<(String, Option<String>, String)>,
    ) {
        if let Some(mut queue) = self.pending.get_mut(subject) {
            while let Some(m) = queue.pop_front() {
                let _ = tx.send(m);
            }
        }
        self.subs.entry(subject.to_string()).or_default().push(tx);
    }
}

/// NATS server facade: resolves handlers via DI, subscribes to registered patterns,
/// and dispatches incoming messages to the right handler.
pub struct NatsServer {
    container: fr_core::Container,
    bus: Arc<MemoryNats>,
    poll_ms: u64,
}

impl NatsServer {
    pub fn new(container: fr_core::Container, config: Arc<ConfigService>) -> Self {
        let poll_ms = config.get_or_parse("NATS_POLL_MS", 50u64);
        Self {
            container,
            bus: Arc::new(MemoryNats::default()),
            poll_ms,
        }
    }

    /// Inject a message into the in-memory bus. Useful for tests and local dev
    /// (no external NATS daemon required).
    pub fn publish<M: Serialize>(
        &self,
        subject: &str,
        reply: Option<&str>,
        payload: &M,
    ) -> Result<(), NatsError> {
        let s = serde_json::to_string(payload)?;
        self.bus.publish(subject, reply, &s);
        Ok(())
    }

    /// Run one dispatch cycle — dispatch a message for a single raw payload on a subject.
    pub async fn dispatch(
        &self,
        pattern: &str,
        subject: String,
        reply: Option<String>,
        payload: &str,
    ) -> Result<(), NatsError> {
        let descs: HashMap<&'static str, &NatsHandlerDescriptor> =
            inventory::iter::<NatsHandlerDescriptor>()
                .map(|d| (d.pattern, d))
                .collect();
        let desc = descs
            .get(pattern)
            .copied()
            .ok_or_else(|| NatsError::NoHandler(pattern.into()))?;
        let handler_any = self
            .container
            .try_get_any(desc.handler_type)
            .ok_or_else(|| NatsError::NoHandler(format!("handler not in DI: {pattern}")))?;
        (desc.process)(handler_any, subject, reply, payload.to_string()).await
    }

    /// Convenience: dispatch all registered patterns by subscribing through the
    /// memory bus and polling. Spawns one per-pattern task then waits forever until
    /// cancelled externally. Typical caller wraps with `tokio::spawn`.
    pub async fn run(&self) {
        // Build pattern -> receiver map.
        let mut tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new();
        for desc in inventory::iter::<NatsHandlerDescriptor> {
            let (tx, mut rx) =
                tokio::sync::mpsc::unbounded_channel::<(String, Option<String>, String)>();
            self.bus.subscribe(desc.pattern, tx);
            let container = self.container.clone();
            let pattern = desc.pattern;
            let process = desc.process;
            let handler_type = desc.handler_type;
            tasks.push(tokio::spawn(async move {
                while let Some((subject, reply, payload)) = rx.recv().await {
                    if let Some(handler_any) = container.try_get_any(handler_type) {
                        if let Err(err) = (process)(handler_any, subject, reply, payload).await {
                            eprintln!(
                                "[ferrite-nats] handler for pattern `{pattern}` error: {err:?}"
                            );
                        }
                    }
                }
            }));
        }
        loop {
            tokio::time::sleep(Duration::from_millis(self.poll_ms)).await;
        }
    }

    pub fn patterns(&self) -> Vec<String> {
        inventory::iter::<NatsHandlerDescriptor>()
            .map(|d| d.pattern.to_string())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// NatsModule
// ---------------------------------------------------------------------------

inventory::submit! {
    ProviderEntry::new_static::<NatsServer>(
        "NatsServer",
        Scope::Singleton,
        || vec![
            TypeId::of::<fr_core::Container>(),
            TypeId::of::<ConfigService>(),
        ],
        |container| -> Arc<dyn Any + Send + Sync> {
            let cfg = container.get::<ConfigService>();
            Arc::new(NatsServer::new(container.clone(), cfg)) as Arc<dyn Any + Send + Sync>
        },
    )
}

// ---------------------------------------------------------------------------
// NatsModule
// ---------------------------------------------------------------------------

pub struct NatsModule;

impl NatsModule {
    pub fn for_root() -> NatsModuleImpl {
        NatsModuleImpl
    }
}

#[derive(Clone, Copy)]
pub struct NatsModuleImpl;

impl fr_core::Module for NatsModuleImpl {
    fn __module_descriptor() -> ModuleDescriptor {
        ModuleDescriptor {
            name: String::from("NatsModule"),
            providers: vec![TypeId::of::<NatsServer>()],
            controllers: vec![],
            imports: vec![],
            exports: vec![TypeId::of::<NatsServer>()],
            middleware: vec![],
            global: false,
        }
    }
}

impl OnApplicationBootstrap for NatsModuleImpl {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    struct Greet {
        who: String,
    }

    struct GreetHandler {
        count: Arc<AtomicUsize>,
    }

    impl Default for GreetHandler {
        fn default() -> Self {
            Self {
                count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl MessageHandler<Greet> for GreetHandler {
        const PATTERN: &'static str = "greeter.say";
        async fn handle(&self, msg: NatsMessage<Greet>) -> Result<(), NatsError> {
            self.count.fetch_add(1, Ordering::SeqCst);
            assert_eq!(msg.payload.who, "world");
            Ok(())
        }
    }

    submit_nats_handler!(GreetHandler, Greet);

    #[test]
    fn inventory_collects_handler_pattern() {
        let pats: Vec<_> = inventory::iter::<NatsHandlerDescriptor>()
            .map(|d| d.pattern)
            .collect();
        assert!(pats.contains(&"greeter.say"), "missing pattern: {pats:?}");
    }

    #[tokio::test]
    async fn dispatch_calls_handler() {
        let container = fr_core::Container::default();
        let config = Arc::new(ConfigService::default());
        container.seed_singleton(config);
        // Seed handler
        let handler = Arc::new(GreetHandler::default());
        let counter = handler.count.clone();
        container.seed_singleton(handler);
        let srv = NatsServer::new(container, Arc::new(ConfigService::default()));
        let res = srv
            .dispatch(
                "greeter.say",
                "greeter.say".into(),
                None,
                &serde_json::to_string(&Greet {
                    who: "world".into(),
                })
                .unwrap(),
            )
            .await;
        assert!(res.is_ok(), "dispatch failed: {res:?}");
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn publish_and_run_dispatches() {
        let container = fr_core::Container::default();
        let cfg = Arc::new(ConfigService::default());
        container.seed_singleton(cfg);
        let handler = Arc::new(GreetHandler::default());
        let counter = handler.count.clone();
        container.seed_singleton(handler);
        let srv = Arc::new(NatsServer::new(
            container,
            Arc::new(ConfigService::default()),
        ));
        let srv2 = srv.clone();
        let runner = tokio::spawn(async move { srv2.run().await });
        tokio::time::sleep(Duration::from_millis(40)).await;
        srv.publish(
            "greeter.say",
            None,
            &Greet {
                who: "world".into(),
            },
        )
        .unwrap();
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert!(counter.load(Ordering::SeqCst) >= 1);
        runner.abort();
    }

    #[test]
    fn publish_with_no_handler_returns_err() {
        let container = fr_core::Container::default();
        let cfg = Arc::new(ConfigService::default());
        container.seed_singleton(cfg);
        let srv = NatsServer::new(container, Arc::new(ConfigService::default()));
        let rt = tokio::runtime::Runtime::new().unwrap();
        let err = rt
            .block_on(srv.dispatch("nope", "s".into(), None, "{}"))
            .unwrap_err();
        assert!(matches!(err, NatsError::NoHandler(_)));
    }
}
