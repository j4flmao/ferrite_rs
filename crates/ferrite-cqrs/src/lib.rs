//! ferrite-cqrs: simple in-memory Command/Query/Event bus using the `ferrite-framework` DI
//! container and the linker-folded `inventory` pattern for handler registration.

use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use fr_core::{Container, ModuleDescriptor, OnApplicationBootstrap, ProviderEntry, Scope};

type AnyArc = Arc<dyn Any + Send + Sync>;
type ShimFuture = std::pin::Pin<Box<dyn Send + Future<Output = ()>>>;
type ShimValueFuture = std::pin::Pin<Box<dyn Send + Future<Output = AnyArc>>>;

// ---------------------------------------------------------------------------
// Message traits
// ---------------------------------------------------------------------------

pub trait Command: Clone + Send + Sync + 'static {}
pub trait Event: Clone + Send + Sync + 'static {}
pub trait Query: Clone + Send + Sync + 'static {
    type Result: Clone + Send + Sync + 'static;
}

// ---------------------------------------------------------------------------
// Handler traits
// ---------------------------------------------------------------------------

#[async_trait]
pub trait CommandHandler<C: Command>: Send + Sync + 'static {
    async fn handle(&self, command: C);
}

#[async_trait]
pub trait QueryHandler<Q: Query>: Send + Sync + 'static {
    async fn handle(&self, query: Q) -> Q::Result;
}

#[async_trait]
pub trait EventHandler<E: Event>: Send + Sync + 'static {
    async fn handle(&self, event: E);
}

// ---------------------------------------------------------------------------
// Handler inventory descriptors (stored in linker-collected inventory slices)
// ---------------------------------------------------------------------------

pub struct CommandHandlerDescriptor {
    pub command_type_id: TypeId,
    pub command_type_name: &'static str,
    pub handler_type_id: TypeId,
    pub handler_type_name: &'static str,
}

pub struct QueryHandlerDescriptor {
    pub query_type_id: TypeId,
    pub query_type_name: &'static str,
    pub result_type_name: &'static str,
    pub handler_type_id: TypeId,
    pub handler_type_name: &'static str,
}

pub struct EventHandlerDescriptor {
    pub event_type_id: TypeId,
    pub event_type_name: &'static str,
    pub handler_type_id: TypeId,
    pub handler_type_name: &'static str,
}

inventory::collect!(CommandHandlerDescriptor);
inventory::collect!(QueryHandlerDescriptor);
inventory::collect!(EventHandlerDescriptor);

/// Register a concrete `CommandHandler<CommandTy>` implementation.
/// Usage: `submit_command_handler!(CreateUserHandler, CreateUser);`
#[macro_export]
macro_rules! submit_command_handler {
    ($HandlerTy:ty, $CommandTy:ty) => {
        $crate::__private_submit_command_handler!($HandlerTy, $CommandTy);
    };
}

/// Register a concrete `QueryHandler<QueryTy>` implementation.
/// Usage: `submit_query_handler!(FindUserHandler, FindUser);`
#[macro_export]
macro_rules! submit_query_handler {
    ($HandlerTy:ty, $QueryTy:ty) => {
        $crate::__private_submit_query_handler!($HandlerTy, $QueryTy);
    };
}

/// Register a concrete `EventHandler<EventTy>` implementation. Fan-out style:
/// multiple handlers per event are supported.
/// Usage: `submit_event_handler!(AuditEventHandler, UserCreated);`
#[macro_export]
macro_rules! submit_event_handler {
    ($HandlerTy:ty, $EventTy:ty) => {
        $crate::__private_submit_event_handler!($HandlerTy, $EventTy);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __private_submit_command_handler {
    ($HandlerTy:ty, $CommandTy:ty) => {
        ::inventory::submit! {
            $crate::CommandHandlerDescriptor {
                command_type_id: ::std::any::TypeId::of::<$CommandTy>(),
                command_type_name: "",
                handler_type_id: ::std::any::TypeId::of::<$HandlerTy>(),
                handler_type_name: "",
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __private_submit_query_handler {
    ($HandlerTy:ty, $QueryTy:ty) => {
        ::inventory::submit! {
            $crate::QueryHandlerDescriptor {
                query_type_id: ::std::any::TypeId::of::<$QueryTy>(),
                query_type_name: "",
                result_type_name: "",
                handler_type_id: ::std::any::TypeId::of::<$HandlerTy>(),
                handler_type_name: "",
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __private_submit_event_handler {
    ($HandlerTy:ty, $EventTy:ty) => {
        ::inventory::submit! {
            $crate::EventHandlerDescriptor {
                event_type_id: ::std::any::TypeId::of::<$EventTy>(),
                event_type_name: "",
                handler_type_id: ::std::any::TypeId::of::<$HandlerTy>(),
                handler_type_name: "",
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Buses
// ---------------------------------------------------------------------------

/// Internal shared index used by the three buses so their inventories are
/// iterated only once when the first dispatch is requested.
#[derive(Default)]
struct CqrsIndex {
    commands: BTreeMap<TypeId, Vec<(TypeId, &'static str)>>,
    queries: BTreeMap<TypeId, (TypeId, &'static str)>,
    events: BTreeMap<TypeId, Vec<(TypeId, &'static str)>>,
}

impl CqrsIndex {
    fn build() -> Self {
        let mut me = Self::default();
        for d in inventory::iter::<CommandHandlerDescriptor> {
            me.commands
                .entry(d.command_type_id)
                .or_default()
                .push((d.handler_type_id, d.handler_type_name));
        }
        for d in inventory::iter::<QueryHandlerDescriptor> {
            me.queries
                .entry(d.query_type_id)
                .or_insert((d.handler_type_id, d.handler_type_name));
        }
        for d in inventory::iter::<EventHandlerDescriptor> {
            me.events
                .entry(d.event_type_id)
                .or_default()
                .push((d.handler_type_id, d.handler_type_name));
        }
        me
    }
}

/// Bus that dispatches a `Command` to exactly one registered handler.
pub struct CommandBus {
    container: Container,
    index: Mutex<Option<Arc<CqrsIndex>>>,
}

impl CommandBus {
    pub fn new(container: Container) -> Self {
        Self {
            container,
            index: Mutex::new(None),
        }
    }

    fn index(&self) -> Arc<CqrsIndex> {
        let mut g = self.index.lock().unwrap();
        if g.is_none() {
            *g = Some(Arc::new(CqrsIndex::build()));
        }
        g.as_ref().unwrap().clone()
    }

    /// Dispatch a command. Returns `Ok(())` if exactly one handler was found
    /// and ran to completion; returns `Err` with a descriptive message if no
    /// handler (or multiple handlers) were registered.
    pub async fn dispatch<C: Command + 'static>(&self, command: C) -> Result<(), String> {
        let idx = self.index();
        let entries = idx
            .commands
            .get(&TypeId::of::<C>())
            .cloned()
            .unwrap_or_default();
        let (handler_id, _name) = match entries.len() {
            1 => entries.into_iter().next().unwrap(),
            0 => {
                return Err(format!(
                    "no CommandHandler registered for {}",
                    std::any::type_name::<C>()
                ))
            }
            n => {
                return Err(format!(
                    "{n} CommandHandlers registered for {}; expected exactly one",
                    std::any::type_name::<C>()
                ))
            }
        };

        let container = self.container.clone();
        let handler = container.try_get_any(handler_id).unwrap_or_else(|| {
            panic!("missing Injectable for handler TypeId={:?}", handler_id);
        });

        let shim = inventory::iter::<CommandHandlerShimEntry>()
            .find(|s| s.command_type_id == TypeId::of::<C>() && s.handler_type_id == handler_id);
        match shim {
            Some(s) => (s.run)(handler, Arc::new(command.clone())).await,
            None => {
                return Err(format!(
                    "missing CommandHandlerShimEntry for {}",
                    std::any::type_name::<C>()
                ));
            }
        }
        Ok(())
    }
}

inventory::collect!(CommandHandlerShimEntry);

pub struct CommandHandlerShimEntry {
    pub command_type_id: TypeId,
    pub handler_type_id: TypeId,
    pub run: fn(AnyArc, AnyArc) -> ShimFuture,
}

// Shim macros: typed bridge between inventory handler and `dyn Any` runtime.
#[doc(hidden)]
#[macro_export]
macro_rules! __private_shim_command {
    ($HandlerTy:ty, $CommandTy:ty) => {
        ::inventory::submit! {
            $crate::CommandHandlerShimEntry {
                command_type_id: ::std::any::TypeId::of::<$CommandTy>(),
                handler_type_id: ::std::any::TypeId::of::<$HandlerTy>(),
                run: |handler_any: ::std::sync::Arc<dyn ::std::any::Any + Send + Sync>,
                      cmd_any: ::std::sync::Arc<dyn ::std::any::Any + Send + Sync>|
                    -> ::std::pin::Pin<Box<dyn Send + ::std::future::Future<Output = ()>>> {
                    let handler = handler_any.downcast::<$HandlerTy>().unwrap();
                    let cmd = (*cmd_any.downcast::<$CommandTy>().unwrap()).clone();
                    Box::pin(async move {
                        <$HandlerTy as $crate::CommandHandler<$CommandTy>>::handle(
                            &*handler,
                            cmd,
                        )
                        .await
                    })
                },
            }
        }
    };
}

pub struct QueryHandlerShimEntry {
    pub query_type_id: TypeId,
    pub handler_type_id: TypeId,
    pub run: fn(AnyArc, AnyArc) -> ShimValueFuture,
}

inventory::collect!(QueryHandlerShimEntry);

/// Shim for QueryHandler dispatch (typed fn pointer stored in inventory).
#[doc(hidden)]
#[macro_export]
macro_rules! __private_shim_query {
    ($HandlerTy:ty, $QueryTy:ty) => {
        ::inventory::submit! {
            $crate::QueryHandlerShimEntry {
                query_type_id: ::std::any::TypeId::of::<$QueryTy>(),
                handler_type_id: ::std::any::TypeId::of::<$HandlerTy>(),
                run: |handler_any: ::std::sync::Arc<dyn ::std::any::Any + Send + Sync>,
                      q_any: ::std::sync::Arc<dyn ::std::any::Any + Send + Sync>|
                    -> ::std::pin::Pin<Box<dyn Send + ::std::future::Future<Output = ::std::sync::Arc<dyn ::std::any::Any + Send + Sync>>>> {
                    let handler = handler_any.downcast::<$HandlerTy>().unwrap();
                    let q = (*q_any.downcast::<$QueryTy>().unwrap()).clone();
                    Box::pin(async move {
                        let r = <$HandlerTy as $crate::QueryHandler<$QueryTy>>::handle(&*handler, q).await;
                        ::std::sync::Arc::new(r) as ::std::sync::Arc<dyn ::std::any::Any + Send + Sync>
                    })
                },
            }
        }
    };
}

pub struct EventHandlerShimEntry {
    pub event_type_id: TypeId,
    pub handler_type_id: TypeId,
    pub run: fn(AnyArc, AnyArc) -> ShimFuture,
}

inventory::collect!(EventHandlerShimEntry);

/// Shim for EventHandler dispatch (typed fn pointer stored in inventory).
#[doc(hidden)]
#[macro_export]
macro_rules! __private_shim_event {
    ($HandlerTy:ty, $EventTy:ty) => {
        ::inventory::submit! {
            $crate::EventHandlerShimEntry {
                event_type_id: ::std::any::TypeId::of::<$EventTy>(),
                handler_type_id: ::std::any::TypeId::of::<$HandlerTy>(),
                run: |handler_any: ::std::sync::Arc<dyn ::std::any::Any + Send + Sync>,
                      ev_any: ::std::sync::Arc<dyn ::std::any::Any + Send + Sync>|
                    -> ::std::pin::Pin<Box<dyn Send + ::std::future::Future<Output = ()>>> {
                    let handler = handler_any.downcast::<$HandlerTy>().unwrap();
                    let ev = (*ev_any.downcast::<$EventTy>().unwrap()).clone();
                    Box::pin(async move {
                        <$HandlerTy as $crate::EventHandler<$EventTy>>::handle(&*handler, ev).await
                    })
                },
            }
        }
    };
}

/// Bus that dispatches a `Query` to a handler and returns its result.
pub struct QueryBus {
    container: Container,
    index: Mutex<Option<Arc<CqrsIndex>>>,
}

impl QueryBus {
    pub fn new(container: Container) -> Self {
        Self {
            container,
            index: Mutex::new(None),
        }
    }

    fn index(&self) -> Arc<CqrsIndex> {
        let mut g = self.index.lock().unwrap();
        if g.is_none() {
            *g = Some(Arc::new(CqrsIndex::build()));
        }
        g.as_ref().unwrap().clone()
    }

    pub async fn dispatch<Q: Query + 'static>(&self, query: Q) -> Result<Q::Result, String> {
        let idx = self.index();
        let (handler_id, _name) =
            idx.queries
                .get(&TypeId::of::<Q>())
                .copied()
                .ok_or_else(|| {
                    format!(
                        "no QueryHandler registered for {}",
                        std::any::type_name::<Q>()
                    )
                })?;

        let container = self.container.clone();
        let handler = container.try_get_any(handler_id).ok_or_else(|| {
            format!(
                "missing Injectable provider for QueryHandler {}",
                std::any::type_name::<Q>()
            )
        })?;

        let shim = inventory::iter::<QueryHandlerShimEntry>()
            .find(|s| s.query_type_id == TypeId::of::<Q>() && s.handler_type_id == handler_id)
            .ok_or_else(|| {
                format!(
                    "missing QueryHandlerShimEntry for {}",
                    std::any::type_name::<Q>()
                )
            })?;
        let any_res = (shim.run)(handler, Arc::new(query)).await;
        match any_res.downcast::<Q::Result>() {
            Ok(arc) => Ok((*arc).clone()),
            Err(_) => Err("QueryBus result layout mismatch".to_string()),
        }
    }
}

/// Bus that dispatches an `Event` to *all* registered handlers (fan-out).
pub struct EventBus {
    container: Container,
    index: Mutex<Option<Arc<CqrsIndex>>>,
}

impl EventBus {
    pub fn new(container: Container) -> Self {
        Self {
            container,
            index: Mutex::new(None),
        }
    }

    fn index(&self) -> Arc<CqrsIndex> {
        let mut g = self.index.lock().unwrap();
        if g.is_none() {
            *g = Some(Arc::new(CqrsIndex::build()));
        }
        g.as_ref().unwrap().clone()
    }

    pub async fn publish<E: Event + 'static>(&self, event: E) -> Result<usize, String> {
        let idx = self.index();
        let entries = idx
            .events
            .get(&TypeId::of::<E>())
            .cloned()
            .unwrap_or_default();
        let container = self.container.clone();
        let ev_arc = Arc::new(event);
        let mut dispatched = 0usize;
        for (handler_id, _name) in entries {
            let handler = match container.try_get_any(handler_id) {
                Some(h) => h,
                None => continue,
            };
            let shim = inventory::iter::<EventHandlerShimEntry>()
                .find(|s| s.event_type_id == TypeId::of::<E>() && s.handler_type_id == handler_id);
            if let Some(s) = shim {
                (s.run)(handler, ev_arc.clone()).await;
                dispatched += 1;
            }
        }
        Ok(dispatched)
    }
}

// ---------------------------------------------------------------------------
// Injectable registrations for CommandBus, QueryBus, EventBus
// ---------------------------------------------------------------------------

inventory::submit! {
    ProviderEntry::new_static::<CommandBus>(
        "CommandBus",
        Scope::Singleton,
        || vec![TypeId::of::<Container>()],
        |container| -> AnyArc {
            Arc::new(CommandBus::new(container.clone())) as AnyArc
        },
    )
}

inventory::submit! {
    ProviderEntry::new_static::<QueryBus>(
        "QueryBus",
        Scope::Singleton,
        || vec![TypeId::of::<Container>()],
        |container| -> AnyArc {
            Arc::new(QueryBus::new(container.clone())) as AnyArc
        },
    )
}

inventory::submit! {
    ProviderEntry::new_static::<EventBus>(
        "EventBus",
        Scope::Singleton,
        || vec![TypeId::of::<Container>()],
        |container| -> AnyArc {
            Arc::new(EventBus::new(container.clone())) as AnyArc
        },
    )
}

// ---------------------------------------------------------------------------
// CqrsModule
// ---------------------------------------------------------------------------

pub struct CqrsModule;

impl CqrsModule {
    pub fn for_root() -> CqrsModuleImpl {
        CqrsModuleImpl
    }
}

#[derive(Clone, Copy)]
pub struct CqrsModuleImpl;

impl fr_core::Module for CqrsModuleImpl {
    fn __module_descriptor() -> ModuleDescriptor {
        ModuleDescriptor {
            name: String::from("CqrsModule"),
            providers: vec![
                TypeId::of::<CommandBus>(),
                TypeId::of::<QueryBus>(),
                TypeId::of::<EventBus>(),
            ],
            controllers: vec![],
            imports: vec![],
            exports: vec![
                TypeId::of::<CommandBus>(),
                TypeId::of::<QueryBus>(),
                TypeId::of::<EventBus>(),
            ],
            middleware: vec![],
            global: false,
        }
    }
}

#[async_trait]
impl OnApplicationBootstrap for CqrsModuleImpl {
    async fn on_application_bootstrap(&self) {
        let _ = Duration::from_millis(0);
    }
}

// ---------------------------------------------------------------------------
// Internal: drop `unused_import` warnings
// ---------------------------------------------------------------------------

pub(crate) fn _cqrs_unused_import_sink() {
    let _ = Duration::from_millis(0);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded_container() -> Container {
        Container::default()
    }

    // ------------------------------------------------------------
    // 1. Inventory collect pattern: >=2 command handler descriptors
    // ------------------------------------------------------------

    #[derive(Clone)]
    struct FakeCommand;
    impl Command for FakeCommand {}
    #[derive(Clone)]
    struct FakeCommand2;
    impl Command for FakeCommand2 {}
    #[derive(Default, Clone)]
    struct FakeCmdHandler;
    #[derive(Default, Clone)]
    struct FakeCmdHandler2;
    #[async_trait]
    impl CommandHandler<FakeCommand> for FakeCmdHandler {
        async fn handle(&self, _c: FakeCommand) {}
    }
    #[async_trait]
    impl CommandHandler<FakeCommand2> for FakeCmdHandler2 {
        async fn handle(&self, _c: FakeCommand2) {}
    }

    submit_command_handler!(FakeCmdHandler, FakeCommand);
    submit_command_handler!(FakeCmdHandler2, FakeCommand2);
    crate::__private_shim_command!(FakeCmdHandler, FakeCommand);
    crate::__private_shim_command!(FakeCmdHandler2, FakeCommand2);

    #[test]
    fn inventory_collect_pattern_at_least_two_command_handlers() {
        let mut n = 0usize;
        for _ in inventory::iter::<CommandHandlerDescriptor> {
            n += 1;
        }
        assert!(n >= 2, "expected >= 2 command handler descriptors, got {n}");
    }

    // ------------------------------------------------------------
    // 2. Dispatch command
    // ------------------------------------------------------------

    #[derive(Clone)]
    struct IncrCmd {
        amount: i32,
    }
    impl Command for IncrCmd {}

    #[derive(Clone)]
    struct IncrCmdHandler {
        sum: Arc<Mutex<i32>>,
    }
    #[async_trait]
    impl CommandHandler<IncrCmd> for IncrCmdHandler {
        async fn handle(&self, cmd: IncrCmd) {
            let mut g = self.sum.lock().unwrap();
            *g += cmd.amount;
        }
    }

    submit_command_handler!(IncrCmdHandler, IncrCmd);
    crate::__private_shim_command!(IncrCmdHandler, IncrCmd);

    #[tokio::test]
    async fn dispatch_command_to_single_handler() {
        let shared = Arc::new(Mutex::new(0));
        let c = seeded_container();
        c.seed_singleton(Arc::new(IncrCmdHandler {
            sum: shared.clone(),
        }));
        let bus = CommandBus::new(c);
        bus.dispatch(IncrCmd { amount: 3 }).await.unwrap();
        bus.dispatch(IncrCmd { amount: 7 }).await.unwrap();
        assert_eq!(*shared.lock().unwrap(), 10);
    }

    // ------------------------------------------------------------
    // 3. Query returns a value
    // ------------------------------------------------------------

    #[derive(Clone)]
    struct CountUsersQ;
    impl Query for CountUsersQ {
        type Result = usize;
    }
    #[derive(Clone, Default)]
    struct CountUsersHandler;
    #[async_trait]
    impl QueryHandler<CountUsersQ> for CountUsersHandler {
        async fn handle(&self, _q: CountUsersQ) -> usize {
            42
        }
    }

    submit_query_handler!(CountUsersHandler, CountUsersQ);
    crate::__private_shim_query!(CountUsersHandler, CountUsersQ);

    #[tokio::test]
    async fn query_returns_handler_result() {
        let c = seeded_container();
        c.seed_singleton(Arc::new(CountUsersHandler));
        let bus = QueryBus::new(c);
        let got = bus.dispatch(CountUsersQ).await.unwrap();
        assert_eq!(got, 42usize);
    }

    // ------------------------------------------------------------
    // 4. Event fan-out to two handlers
    // ------------------------------------------------------------

    #[derive(Clone)]
    struct UserCreatedEv {
        id: u64,
    }
    impl Event for UserCreatedEv {}

    #[derive(Clone)]
    struct AuditHandler {
        log: Arc<Mutex<Vec<u64>>>,
    }
    #[derive(Clone)]
    struct WelcomeHandler {
        sent: Arc<Mutex<Vec<u64>>>,
    }
    #[async_trait]
    impl EventHandler<UserCreatedEv> for AuditHandler {
        async fn handle(&self, ev: UserCreatedEv) {
            self.log.lock().unwrap().push(ev.id);
        }
    }
    #[async_trait]
    impl EventHandler<UserCreatedEv> for WelcomeHandler {
        async fn handle(&self, ev: UserCreatedEv) {
            self.sent.lock().unwrap().push(ev.id);
        }
    }

    submit_event_handler!(AuditHandler, UserCreatedEv);
    submit_event_handler!(WelcomeHandler, UserCreatedEv);
    crate::__private_shim_event!(AuditHandler, UserCreatedEv);
    crate::__private_shim_event!(WelcomeHandler, UserCreatedEv);

    #[tokio::test]
    async fn event_fan_out_dispatches_to_both_handlers() {
        let c = seeded_container();
        let audit_log = Arc::new(Mutex::new(Vec::<u64>::new()));
        let welcome_sent = Arc::new(Mutex::new(Vec::<u64>::new()));
        c.seed_singleton(Arc::new(AuditHandler {
            log: audit_log.clone(),
        }));
        c.seed_singleton(Arc::new(WelcomeHandler {
            sent: welcome_sent.clone(),
        }));
        let bus = EventBus::new(c);
        let n = bus.publish(UserCreatedEv { id: 101 }).await.unwrap();
        assert_eq!(n, 2);
        assert_eq!(audit_log.lock().unwrap().as_slice(), &[101]);
        assert_eq!(welcome_sent.lock().unwrap().as_slice(), &[101]);
    }
}
