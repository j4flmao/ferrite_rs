//! Request pipeline primitives: guards, interceptors, middleware, pipes and
//! exception filters, executed in Nest order:
//!
//! ```text
//! Middleware → Guards → Interceptors(before) → Pipes → Handler
//!     → Interceptors(after) → [Exception Filter if error] → Response
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::HeaderName;
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method, Uri};
use axum::middleware::Next as AxumNext;
use axum::response::{IntoResponse, Response};

use crate::HttpError;

/// Wraps the incoming request so guards/interceptors/middleware can inspect
/// it before the handler runs. Roughly Nest's `@Req()`. Only the request's
/// [`Parts`] are kept (method/uri/headers/extensions, all `Send + Sync`); the
/// body travels inside [`Next`] and is re-attached when the handler runs.
pub struct RequestCtx(pub Parts);

impl RequestCtx {
    pub fn new(parts: Parts) -> Self {
        Self(parts)
    }

    pub fn method(&self) -> &Method {
        &self.0.method
    }

    pub fn uri(&self) -> &Uri {
        &self.0.uri
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.0.headers
    }

    /// Read a header by name (`"authorization"`, `Host`, ...).
    pub fn header(&self, name: impl TryInto<HeaderName>) -> Option<&axum::http::HeaderValue> {
        let name: HeaderName = name.try_into().ok()?;
        self.0.headers.get(name)
    }

    /// Convenience: header value as an owned string.
    pub fn header_str(&self, name: impl TryInto<HeaderName>) -> Option<String> {
        let value = self.header(name)?;
        String::from_utf8(value.as_bytes().to_vec()).ok()
    }

    /// Re-attach the body to rebuild the full request.
    pub fn into_http_request(self, body: Body) -> Request {
        Request::from_parts(self.0, body)
    }
}

/// The continuation passed to middleware/interceptors. Calling `run` executes
/// the rest of the pipeline (inner hops, then the route handler).
pub struct Next {
    raw: AxumNext,
    hops: Vec<Arc<dyn Hop>>,
    body: Body,
}

impl Next {
    pub(crate) fn new(raw: AxumNext, hops: Vec<Arc<dyn Hop>>, body: Body) -> Self {
        Self { raw, hops, body }
    }

    pub async fn run(self, ctx: RequestCtx) -> Response {
        let mut hops = self.hops.into_iter();
        match hops.next() {
            Some(hop) => {
                let inner = Next {
                    raw: self.raw,
                    hops: hops.collect(),
                    body: self.body,
                };
                hop.call(ctx, inner).await
            }
            None => self.raw.run(ctx.into_http_request(self.body)).await,
        }
    }
}

/// Internal link in the interceptor/middleware chain.
#[async_trait]
pub trait Hop: Send + Sync + 'static {
    async fn call(&self, ctx: RequestCtx, next: Next) -> Response;
}

/// Nest `Guard` — runs before interceptors/pipes/handler; `false` short-circuits
/// with 403 Forbidden.
#[async_trait]
pub trait Guard: Send + Sync + 'static {
    async fn can_activate(&self, ctx: &RequestCtx) -> bool;
}

/// Nest `Interceptor` — wraps the handler on both sides.
#[async_trait]
pub trait Interceptor: Send + Sync + 'static {
    async fn intercept(&self, ctx: RequestCtx, next: Next) -> Response;
}

struct InterceptorHop(Arc<dyn Interceptor>);

#[async_trait]
impl Hop for InterceptorHop {
    async fn call(&self, ctx: RequestCtx, next: Next) -> Response {
        self.0.intercept(ctx, next).await
    }
}

/// Nest `Middleware` — coarser, Nest's middleware has no typed continuation
/// but Ferrite gives it one so the API stays uniform.
#[async_trait]
pub trait Middleware: Send + Sync + 'static {
    async fn handle(&self, ctx: RequestCtx, next: Next) -> Response;
}

struct MiddlewareHop(Arc<dyn Middleware>);

#[async_trait]
impl Hop for MiddlewareHop {
    async fn call(&self, ctx: RequestCtx, next: Next) -> Response {
        self.0.handle(ctx, next).await
    }
}

/// Error raised by a failing [`Pipe`].
#[derive(Debug, thiserror::Error)]
pub enum PipeError {
    #[error("{0}")]
    Message(String),
    /// A pipe produced a fully-formed response (e.g. a structured 422 from
    /// the validation pipe).
    #[error("pipe produced a custom response")]
    Response(Box<axum::response::Response>),
}

impl PipeError {
    pub fn new(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    /// Wrap a fully-formed [`axum::response::Response`] as a pipe error.
    pub fn response(response: axum::response::Response) -> Self {
        Self::Response(Box::new(response))
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    pub fn into_http(self) -> HttpError {
        match self {
            Self::Message(m) => HttpError::bad_request(m),
            Self::Response(_) => HttpError::internal("pipe returned a raw response"),
        }
    }
}

/// Nest `Pipe` — transforms/validates a value before the handler sees it.
pub trait Pipe<Input>: Send + Sync + 'static {
    type Output;
    fn transform(&self, value: Input) -> Result<Self::Output, PipeError>;
}

/// Built-in: parse a string to `i64`.
pub struct ParseIntPipe;

impl Pipe<String> for ParseIntPipe {
    type Output = i64;
    fn transform(&self, value: String) -> Result<i64, PipeError> {
        value
            .parse()
            .map_err(|_| PipeError::bad_request("not an integer"))
    }
}

/// Built-in: substitute a default when the incoming value is empty.
pub struct DefaultValuePipe(pub String);

impl Pipe<String> for DefaultValuePipe {
    type Output = String;
    fn transform(&self, value: String) -> Result<String, PipeError> {
        if value.is_empty() {
            Ok(self.0.clone())
        } else {
            Ok(value)
        }
    }
}

/// Nest `ExceptionFilter` — converts an [`HttpError`] into a custom response.
pub trait ExceptionFilter: Send + Sync + 'static {
    fn catch(&self, err: HttpError) -> Response;
}

/// The state carried by the generated route middleware: resolved guards,
/// interceptors, middleware and exception filters.
#[derive(Clone, Default)]
pub struct RoutePipeline {
    pub guards: Vec<Arc<dyn Guard>>,
    pub hops: Vec<Arc<dyn Hop>>,
    pub filters: Vec<Arc<dyn ExceptionFilter>>,
}

impl RoutePipeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn guard(mut self, guard: Arc<dyn Guard>) -> Self {
        self.guards.push(guard);
        self
    }

    pub fn interceptor(mut self, interceptor: Arc<dyn Interceptor>) -> Self {
        self.hops.push(Arc::new(InterceptorHop(interceptor)));
        self
    }

    pub fn middleware(mut self, middleware: Arc<dyn Middleware>) -> Self {
        self.hops.push(Arc::new(MiddlewareHop(middleware)));
        self
    }

    pub fn filter(mut self, filter: Arc<dyn ExceptionFilter>) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.guards.is_empty() && self.hops.is_empty() && self.filters.is_empty()
    }

    /// Execute guards, then the hop chain, then run exception filters on
    /// error responses. Use with `axum::middleware::from_fn_with_state`.
    pub async fn run(
        State(state): State<RoutePipeline>,
        request: Request,
        next: AxumNext,
    ) -> Response {
        let mut state = state;

        let (parts, body) = request.into_parts();
        let ctx = RequestCtx::new(parts);

        for guard in &state.guards {
            if !guard.can_activate(&ctx).await {
                return HttpError::forbidden("forbidden by guard").into_response();
            }
        }

        let hops = std::mem::take(&mut state.hops);
        let chain = Next::new(next, hops, body);
        let res = chain.run(ctx).await;

        if !state.filters.is_empty()
            && (res.status().is_client_error() || res.status().is_server_error())
        {
            let status = res.status();
            let message = "request failed".to_string();
            let err = HttpError::new(status, message);
            if let Some(filter) = state.filters.first() {
                return filter.catch(err.clone());
            }
        }

        res
    }
}
