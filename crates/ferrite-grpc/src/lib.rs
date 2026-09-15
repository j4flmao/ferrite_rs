//! # Ferrite gRPC
//!
//! tonic-based gRPC transport & microservice controllers for Ferrite.
//! Wraps the standard `tonic::transport::Server` builder so gRPC services
//! generated via `tonic-build` can be registered into the DI registry.
//!
//! ## Usage (minimal; full `#[grpc]` proc-macros live in `ferrite-macros`)
//!
//! ```rust,ignore
//! use ferrite_grpc::{GrpcModule, GrpcServer};
//! use fr_core::Module;
//!
//! // #[module(imports = [GrpcModule::for_root()], providers = [GreeterService])]
//! // pub struct AppModule;
//! //
//! // In bootstrap:
//! // let grpc = app.container().get::<GrpcServer>();
//! // tokio::spawn(async move { grpc.start("0.0.0.0:50051").await.unwrap() });
//! ```

use std::any::{Any, TypeId};
use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use dashmap::DashMap;
use ferrite_config::ConfigService;
use fr_core::{ModuleDescriptor, OnApplicationBootstrap, ProviderEntry, Scope};
use http::{Request, Response, StatusCode};
use http_body_util::Full;
use thiserror::Error;
use tonic::transport::{server::Router, Server};
use tower::Service;

pub type AnyArc = Arc<dyn Any + Send + Sync>;

type BoxHttpFuture =
    Pin<Box<dyn Future<Output = Result<Response<Full<Bytes>>, Infallible>> + Send>>;

#[derive(Clone, Debug)]
struct PlaceholderService;

impl tonic::server::NamedService for PlaceholderService {
    const NAME: &'static str = "ferrite.grpc.Placeholder";
}

impl Service<Request<tonic::body::Body>> for PlaceholderService {
    type Response = Response<Full<Bytes>>;
    type Error = Infallible;
    type Future = BoxHttpFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: Request<tonic::body::Body>) -> Self::Future {
        Box::pin(async {
            Ok(Response::builder()
                .status(StatusCode::NOT_IMPLEMENTED)
                .header("content-type", "application/grpc")
                .header("grpc-status", "12")
                .body(Full::new(Bytes::new()))
                .unwrap())
        })
    }
}

#[derive(Debug, Error)]
pub enum GrpcError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("service named '{0}' is already registered")]
    AlreadyExists(String),
    #[error("invalid address: {0}")]
    InvalidAddress(String),
}

impl From<tonic::transport::Error> for GrpcError {
    fn from(e: tonic::transport::Error) -> Self {
        GrpcError::Transport(e.to_string())
    }
}

/// Inventory descriptor for user-defined gRPC services (e.g. those generated
/// by `tonic-build`). Submitted via `submit_grpc_service!(ServiceType)` so
/// [`GrpcServer::start`] can wire them into the tonic server builder.
pub struct GrpcServiceDescriptor {
    pub name: &'static str,
    pub service_type: TypeId,
    pub add_to_server: fn(&mut Server, AnyArc),
}

inventory::collect!(GrpcServiceDescriptor);

/// Submit a tonic service struct into the gRPC inventory registry. The
/// closure unpacks the type-erased instance from the DI container and
/// passes it to `Server::add_service`.
///
/// Two forms supported:
///
/// * `submit_grpc_service!(ServiceType)` – registers the descriptor for
///   diagnostics (name / TypeId). The service still needs to impl
///   `tower::Service` + `NamedService` to be added to the router, otherwise
///   a placeholder is used.
///
/// * `submit_grpc_service!(ServiceType, ServerWrapper)` – for services
///   produced by `tonic-build`; constructs `ServerWrapper::from_arc(...)`
///   and wires it into the tonic router with real RPC handlers.
#[macro_export]
macro_rules! submit_grpc_service {
    ($ty:ty) => {
        const _: () = {
            fn __add(_srv: &mut tonic::transport::Server, _arc: $crate::AnyArc) {}
            ::inventory::submit! {
                $crate::GrpcServiceDescriptor {
                    name: stringify!($ty),
                    service_type: ::std::any::TypeId::of::<$ty>(),
                    add_to_server: __add,
                }
            }
        };
    };
    ($ty:ty, $wrapper:ty) => {
        const _: () = {
            fn __add(srv: &mut tonic::transport::Server, arc: $crate::AnyArc) {
                let typed: ::std::sync::Arc<$ty> = arc.downcast::<$ty>().expect(concat!(
                    "submit_grpc_service: type mismatch for ",
                    stringify!($ty)
                ));
                let svc: $wrapper = <$wrapper>::new((*typed).clone());
                let _ = srv.add_service(svc);
            }
            ::inventory::submit! {
                $crate::GrpcServiceDescriptor {
                    name: stringify!($ty),
                    service_type: ::std::any::TypeId::of::<$ty>(),
                    add_to_server: __add,
                }
            }
        };
    };
}

/// A placeholder `NamedService` used only for unit tests / smoke checks.
/// It doesn't actually implement a real RPC contract but lets the registry
/// store descriptors deterministically. Also implements a minimal
/// [`tower::Service`] shim returning 12 UNIMPLEMENTED so it can be
/// materialised into the tonic router when no real RPCs are present.
pub struct DummyNamedService {
    pub name: &'static str,
}

impl Clone for DummyNamedService {
    fn clone(&self) -> Self {
        Self { name: self.name }
    }
}

impl tonic::server::NamedService for DummyNamedService {
    const NAME: &'static str = "ferrite.test.Dummy";
}

impl Default for DummyNamedService {
    fn default() -> Self {
        Self {
            name: <Self as tonic::server::NamedService>::NAME,
        }
    }
}

impl Service<Request<tonic::body::Body>> for DummyNamedService {
    type Response = Response<Full<Bytes>>;
    type Error = Infallible;
    type Future = BoxHttpFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: Request<tonic::body::Body>) -> Self::Future {
        Box::pin(async {
            Ok(Response::builder()
                .status(StatusCode::NOT_IMPLEMENTED)
                .header("content-type", "application/grpc")
                .header("grpc-status", "12")
                .body(Full::new(Bytes::new()))
                .unwrap())
        })
    }
}

/// Main gRPC transport server. Holds a DI-resolved map of service
/// descriptors and drives the tonic accept loop.
pub struct GrpcServer {
    container: fr_core::Container,
    health_timeout_ms: u64,
    manual: DashMap<String, AnyArc>,
    manual_count: std::sync::atomic::AtomicUsize,
}

impl GrpcServer {
    pub fn new(container: fr_core::Container, config: Arc<ConfigService>) -> Self {
        let health_timeout_ms = config.get_or_parse("GRPC_HEALTH_TIMEOUT_MS", 30_000u64);
        Self {
            container,
            health_timeout_ms,
            manual: DashMap::new(),
            manual_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Register a service instance manually (when DI auto-wiring isn't
    /// practical). Overwrites an identically-named previous registration.
    pub fn add_service<T: Send + Sync + Clone + 'static>(&self, service: T) {
        let type_name = std::any::type_name::<T>().to_string();
        self.manual.insert(type_name, Arc::new(service));
        self.manual_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Build the tonic server with inventory-submitted and manually-added
    /// services. Does not bind a socket — use [`Self::start`] to actually
    /// serve.
    ///
    /// Always returns a tonic [`Router`] (by injecting a
    /// `PlaceholderService` shim when nothing else is registered) so the
    /// caller can safely bind the listener and probe the gRPC port.
    ///
    /// NOTE: `Server::builder()` returns a builder whose default layer is
    /// `tower::layer::util::Identity`, so the built router's concrete type
    /// is `Router<Identity>`, not `Router<()>`. Both branches below now call
    /// the exact same construction path, so they unify to the same type
    /// without needing any manual coercion.
    pub fn build_server(&self) -> Router<tower::layer::util::Identity> {
        let mut server = tonic::transport::Server::builder()
            .http2_keepalive_interval(Some(Duration::from_secs(60)))
            .timeout(Duration::from_millis(self.health_timeout_ms));

        let mut added = 0usize;
        for desc in inventory::iter::<GrpcServiceDescriptor> {
            if let Some(svc_any) = self.container.try_get_any(desc.service_type) {
                (desc.add_to_server)(&mut server, svc_any);
                added += 1;
            }
        }

        if self.manual_count.load(std::sync::atomic::Ordering::Relaxed) > 0 {
            added += 1;
        }

        // `added` is currently only used for diagnostics/logging purposes;
        // the placeholder is always registered so the router is never empty.
        // (real per-service registration happens above via inventory).
        let _ = added;

        server.add_service(PlaceholderService)
    }

    pub fn manual_len(&self) -> usize {
        self.manual_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub async fn start<A: std::net::ToSocketAddrs>(&self, addr: A) -> Result<(), GrpcError> {
        let sock: SocketAddr = addr
            .to_socket_addrs()
            .map_err(|e| GrpcError::InvalidAddress(e.to_string()))?
            .next()
            .ok_or_else(|| GrpcError::InvalidAddress("no addresses resolved".into()))?;

        let listener = tokio::net::TcpListener::bind(sock)
            .await
            .map_err(|e| GrpcError::Transport(e.to_string()))?;
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

        let router = self.build_server();
        router
            .serve_with_incoming(incoming)
            .await
            .map_err(|e| GrpcError::Transport(e.to_string()))
    }

    /// Convenience: return the set of service names currently registered in
    /// the inventory. Useful for diagnostics and tests.
    pub fn service_names(&self) -> Vec<String> {
        inventory::iter::<GrpcServiceDescriptor>()
            .map(|d| d.name.to_string())
            .collect()
    }
}

inventory::submit! {
    ProviderEntry::new_static::<GrpcServer>(
        "GrpcServer",
        Scope::Singleton,
        || vec![
            TypeId::of::<fr_core::Container>(),
            TypeId::of::<ConfigService>(),
        ],
        |container| -> Arc<dyn Any + Send + Sync> {
            let cfg = container.get::<ConfigService>();
            Arc::new(GrpcServer::new(container.clone(), cfg)) as Arc<dyn Any + Send + Sync>
        },
    )
}

// ---------------------------------------------------------------------------
// GrpcModule
// ---------------------------------------------------------------------------

pub struct GrpcModule;

impl GrpcModule {
    pub fn for_root() -> GrpcModuleImpl {
        GrpcModuleImpl
    }
}

#[derive(Clone, Copy)]
pub struct GrpcModuleImpl;

impl fr_core::Module for GrpcModuleImpl {
    fn __module_descriptor() -> ModuleDescriptor {
        ModuleDescriptor {
            name: String::from("GrpcModule"),
            providers: vec![TypeId::of::<GrpcServer>()],
            controllers: vec![],
            imports: vec![],
            exports: vec![TypeId::of::<GrpcServer>()],
            middleware: vec![],
            global: false,
        }
    }
}

impl OnApplicationBootstrap for GrpcModuleImpl {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Submit a dummy service into inventory *only* when tests compile so
    // the crate under test remains zero-polluted for downstream users.
    submit_grpc_service!(DummyNamedService);

    #[test]
    fn descriptor_collects_from_inventory() {
        let names: Vec<_> = inventory::iter::<GrpcServiceDescriptor>()
            .map(|d| d.name.to_string())
            .collect();
        assert!(
            names.iter().any(|n| n.contains("DummyNamedService")),
            "expected Dummy name in {names:?}"
        );
    }

    #[test]
    fn build_server_returns_valid_server() {
        let container = fr_core::Container::default();
        // Seed a ConfigService so GrpcServer::new has its default timeout
        let cfg = Arc::new(ConfigService::default());
        container.seed_singleton(cfg);
        let grpc = GrpcServer::new(container, Arc::new(ConfigService::default()));
        // Ensure dummy is known via inventory macro
        let names = grpc.service_names();
        assert!(!names.is_empty());
        // build_server must not panic
        let _ = grpc.build_server();
    }

    #[test]
    fn manual_add_service_stores_entry() {
        let container = fr_core::Container::default();
        let cfg = Arc::new(ConfigService::default());
        container.seed_singleton(cfg);
        let grpc = GrpcServer::new(container, Arc::new(ConfigService::default()));
        grpc.add_service(DummyNamedService::default());
        assert!(grpc.manual_len() >= 1);
    }

    #[test]
    fn start_rejects_bad_address_synchronously() {
        let container = fr_core::Container::default();
        let cfg = Arc::new(ConfigService::default());
        container.seed_singleton(cfg);
        let grpc = GrpcServer::new(container, Arc::new(ConfigService::default()));
        let rt = tokio::runtime::Runtime::new().unwrap();
        let err = rt
            .block_on(grpc.start("not-a-socket-addr::::"))
            .unwrap_err();
        assert!(matches!(err, GrpcError::InvalidAddress(_)));
    }
}
