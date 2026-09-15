//! Testing utilities for Ferrite:
//!
//! * [`TestingModule`] — builder that wraps your root module and lets you
//!   inject override providers (mocks, deterministic fixtures, etc.) before
//!   the DI graph resolves.
//! * [`TestApp`] — spawns the compiled Ferrite app on a random free loopback
//!   port, and exposes `get_json` / `post_json` / `put` / `delete` /
//!   `raw` helpers over a live `reqwest` client so integration tests feel
//!   just like supertest / NestJS `@nestjs/testing`.
//!
//! # Example
//!
//! ```ignore
//! use ferrite_macros::module;
//! use ferrite_testing::{TestApp, TestingModule};
//!
//! # #[module]
//! # pub struct AppModule;
//! #
//! #[tokio::test]
//! async fn my_test() {
//!     let app = TestingModule::new::<AppModule>().compile().await;
//!     let res = app.get::<serde_json::Value>("/health").await.unwrap();
//!     assert_eq!(res.status(), 200);
//! }
//! ```

use std::sync::Arc;
use std::time::Duration;

use ferrite_framework::Module;
use fr_core::Container;
use reqwest::{Client, Method, RequestBuilder, Response as Resp, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

#[derive(Debug, Error)]
pub enum TestError {
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("invalid response status: {0}")]
    Status(u16),
    #[error("could not parse body JSON: {0}")]
    Parse(String),
    #[error("bind test listener: {0}")]
    Bind(std::io::Error),
    #[error("serve: {0}")]
    Serve(std::io::Error),
}

impl From<reqwest::Error> for TestError {
    fn from(e: reqwest::Error) -> Self {
        TestError::Http(e.to_string())
    }
}
impl From<serde_json::Error> for TestError {
    fn from(e: serde_json::Error) -> Self {
        TestError::Parse(e.to_string())
    }
}

/// A test response — owns its HTTP status code plus a cached response body
/// (JSON or raw bytes).
pub struct TestResponse {
    status: StatusCode,
    bytes: bytes::Bytes,
}

impl TestResponse {
    pub fn status(&self) -> u16 {
        self.status.as_u16()
    }
    pub fn status_code(&self) -> StatusCode {
        self.status
    }
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }
    pub fn body_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
    pub fn json<T: DeserializeOwned>(self) -> Result<T, TestError> {
        Ok(serde_json::from_slice(&self.bytes)?)
    }
    pub fn json_value(&self) -> Result<serde_json::Value, TestError> {
        Ok(serde_json::from_slice(&self.bytes)?)
    }
}

fn reqwest_client_for_tests() -> Client {
    Client::builder()
        .danger_accept_invalid_certs(true) // test loopback only
        .timeout(Duration::from_secs(10))
        .build()
        .expect("test reqwest client builds")
}

/// Builder for compiling a Ferrite app with provider overrides.
pub struct TestingModule {
    seeds: Vec<SeedFn>,
}

type SeedFn = Box<dyn FnOnce(&Container) + Send + 'static>;

impl TestingModule {
    /// Start building a test harness rooted at `M` (your application module).
    pub fn new<M: Module + 'static>() -> Self {
        let _ = std::marker::PhantomData::<M>;
        Self { seeds: Vec::new() }
    }

    /// Inject a specific instance of `T` into the container *before* any
    /// factory runs. Every downstream consumer of `T` will receive this
    /// instance — perfect for mocking heavy infra (DBs, HTTP clients, etc.)
    pub fn override_provider<T: Send + Sync + 'static>(mut self, value: T) -> Self {
        let arc = Arc::new(value);
        self.seeds.push(Box::new(move |c: &Container| {
            c.seed_singleton::<T>(arc);
        }));
        self
    }

    /// Same as [`Self::override_provider`] but accepts a pre-wrapped `Arc<T>`,
    /// handy when you want the test to hold the same `Arc` for assertions.
    pub fn override_provider_arc<T: Send + Sync + 'static>(mut self, arc: Arc<T>) -> Self {
        self.seeds.push(Box::new(move |c: &Container| {
            c.seed_singleton::<T>(arc);
        }));
        self
    }

    /// Convenience: override `ConfigService` with specific env vars written
    /// to a temp `.env`. Uses `ConfigService::load_from` semantics.
    pub fn with_env_vars(mut self, vars: &[(&str, &str)]) -> Self {
        let pairs: Vec<(String, String)> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self.seeds.push(Box::new(move |c: &Container| {
            let dir = std::env::temp_dir().join(format!(
                "ferrite-testing-env-{}-{}",
                std::process::id(),
                rand_suffix()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).ok();
            let file: String = pairs.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
            std::fs::write(dir.join(".env"), file).ok();
            let cfg = ferrite_config::ConfigService::load_from(&dir);
            let _ = std::fs::remove_dir_all(&dir);
            c.seed_singleton::<ferrite_config::ConfigService>(Arc::new(cfg));
        }));
        self
    }

    /// Resolve the DI graph and boot a [`TestApp`] on `127.0.0.1:<random>`.
    /// The returned handle runs the axum server in a background Tokio task
    /// and cancels it on drop (via shutdown signal).
    pub async fn compile<M: Module + 'static>(self) -> TestApp {
        let Self { seeds } = self;
        let app = ferrite_framework::Ferrite::create_with_seed::<M, _>(move |c| {
            for s in seeds {
                s(c);
            }
        })
        .await;

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind random port");
        let addr = listener.local_addr().unwrap();
        let port = addr.port();
        let router = app.router;
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();

        let handle: JoinHandle<()> = tokio::spawn(async move {
            let service = router.into_make_service();
            let graceful = async move {
                let _ = rx.await;
            };
            let server = axum::serve(listener, service);
            let _ = server.with_graceful_shutdown(graceful).await;
        });

        TestApp {
            port,
            container: app.container,
            client: reqwest_client_for_tests(),
            shutdown: Some(tx),
            handle: Some(handle),
        }
    }
}

fn rand_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    t ^ std::process::id() as u64
}

/// Running Ferrite test harness: live HTTP server on a random port.
pub struct TestApp {
    port: u16,
    container: Container,
    client: Client,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl TestApp {
    /// Random loopback port the server is listening on.
    pub fn port(&self) -> u16 {
        self.port
    }
    /// Base URL pointing at the server (e.g. `http://127.0.0.1:34567`).
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
    /// Resolve a provider `T` from the built container, just like your
    /// controllers would.
    pub fn get_provider<T: Send + Sync + 'static>(&self) -> Arc<T> {
        self.container.get::<T>()
    }
    /// Access the raw DI container.
    pub fn container(&self) -> &Container {
        &self.container
    }

    fn req(&self, method: Method, path: &str) -> RequestBuilder {
        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        self.client
            .request(method, format!("{}{}", self.base_url(), path))
    }

    async fn send_req(req: RequestBuilder) -> Result<TestResponse, TestError> {
        let r: Resp = req.send().await?;
        let status = r.status();
        let bytes = r
            .bytes()
            .await
            .map_err(|e| TestError::Http(e.to_string()))?;
        Ok(TestResponse { status, bytes })
    }

    // ── verb helpers ──────────────────────────────────────────────────
    pub async fn get(&self, path: &str) -> Result<TestResponse, TestError> {
        Self::send_req(self.req(Method::GET, path)).await
    }
    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<(u16, T), TestError> {
        let res = self.get(path).await?;
        let status = res.status();
        let body = res.json()?;
        Ok((status, body))
    }

    pub async fn post_json<T: Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<TestResponse, TestError> {
        Self::send_req(self.req(Method::POST, path).json(body)).await
    }
    pub async fn post_json_into<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<(u16, R), TestError> {
        let res = self.post_json(path, body).await?;
        let status = res.status();
        let out = res.json()?;
        Ok((status, out))
    }

    pub async fn put_json<T: Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<TestResponse, TestError> {
        Self::send_req(self.req(Method::PUT, path).json(body)).await
    }

    pub async fn patch_json<T: Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<TestResponse, TestError> {
        Self::send_req(self.req(Method::PATCH, path).json(body)).await
    }

    pub async fn delete(&self, path: &str) -> Result<TestResponse, TestError> {
        Self::send_req(self.req(Method::DELETE, path)).await
    }

    /// Send a raw `reqwest::RequestBuilder` you built yourself (headers,
    /// auth, multipart, etc.). The URL is automatically set to the test
    /// server base URL — just provide a path.
    pub async fn send_custom(
        &self,
        build: impl FnOnce(RequestBuilder) -> RequestBuilder,
        method: Method,
        path: &str,
    ) -> Result<TestResponse, TestError> {
        let rb = build(self.req(method, path));
        Self::send_req(rb).await
    }

    /// Explicit shutdown. If you don't call this, it runs at drop anyway.
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(h) = self.handle.take() {
            h.abort();
        }
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ---------------------------------------------------------------------------
// Pure unit tests (no live server) + an integration test that spins up
// `ferrite-health`'s controller just to cover the HTTP path.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_macros::{controller, impl_controller, inject, injectable, module};

    // ---- Fixtures: a tiny module with an injectable GreetingService + one
    //      controller `/hello` that says "hello <name>". We'll override the
    //      service and assert the override bubbles through to the HTTP layer.

    #[injectable]
    pub struct GreetingService {
        pub name: String,
        _anchor: u8,
    }
    impl GreetingService {
        #[inject]
        pub fn new(name: String) -> Self {
            let _ = std::marker::PhantomData::<String>;
            Self {
                name,
                _anchor: Arc::new(0),
            }
        }
    }

    #[controller("")]
    pub struct HelloController {
        svc: GreetingService,
    }
    #[impl_controller]
    impl HelloController {
        #[inject]
        pub fn new(svc: GreetingService) -> Self {
            Self { svc }
        }
        #[ferrite_macros::get("/hello")]
        pub async fn hello(&self) -> ferrite_framework::extract::Json<String> {
            ferrite_framework::extract::Json(format!("Hello, {}", self.svc.name))
        }
    }

    #[module(
        controllers = [HelloController],
        providers = [GreetingService],
    )]
    pub struct TestAppModule;

    // `GreetingService::new` can't resolve `String` from DI (no provider).
    // We *must* always override it; that is exactly what the override
    // semantics are meant to guarantee in tests.

    #[tokio::test]
    async fn override_provider_replaces_service() {
        let app = TestingModule::new::<TestAppModule>()
            .override_provider::<GreetingService>(GreetingService {
                name: Arc::new("Overridden".to_string()),
                _anchor: Arc::new(0),
            })
            .compile::<TestAppModule>()
            .await;
        let got = app.get_provider::<GreetingService>();
        assert_eq!(got.name.as_str(), "Overridden");
    }

    #[tokio::test]
    async fn http_get_uses_overridden_service_end_to_end() {
        let app = TestingModule::new::<TestAppModule>()
            .override_provider::<GreetingService>(GreetingService {
                name: Arc::new("Charlie".to_string()),
                _anchor: Arc::new(0),
            })
            .compile::<TestAppModule>()
            .await;

        // Retry a few times because axum might not yet be listening
        // (the Tokio task starts concurrently).
        let mut last_err: Option<TestError> = None;
        for _ in 0..30 {
            match app.get("/hello").await {
                Ok(res) => {
                    assert_eq!(res.status(), 200);
                    let s: String = res.json().unwrap();
                    assert_eq!(s, "Hello, Charlie");
                    return;
                }
                Err(e) => {
                    last_err = Some(e);
                    tokio::time::sleep(Duration::from_millis(40)).await;
                }
            }
        }
        panic!(
            "server never became reachable: last_err={}",
            last_err.map(|e| e.to_string()).unwrap_or_default()
        );
    }

    // Pure logic tests — no server

    #[test]
    fn base_url_construction() {
        // Can't construct without a listener — mock the fields only for a
        // URL check by building a minimal struct with zeroed fields (except
        // port). Use unsafe pointer writes + MaybeUninit to avoid leaking.
        use std::mem::MaybeUninit;
        let mut slots: MaybeUninit<TestApp> = MaybeUninit::zeroed();
        // Safety: we only set the u16 and never use the other fields.
        unsafe {
            let ptr = slots.as_mut_ptr();
            std::ptr::addr_of_mut!((*ptr).port).write(1234);
            let s: &TestApp = &*ptr;
            assert_eq!(s.base_url(), "http://127.0.0.1:1234");
        }
    }

    #[test]
    fn test_response_json_helper() {
        let resp = TestResponse {
            status: StatusCode::OK,
            bytes: bytes::Bytes::from_static(b"{\"a\":1}"),
        };
        let v: serde_json::Value = resp.json().unwrap();
        assert_eq!(v["a"], 1);
    }
}
