//! Ferrite HTTP transport — an Axum binding plus the global route registry.
//!
//! The `#[impl_controller]` macro registers `ControllerRoutes` values here via
//! `inventory`. At bootstrap the kernel walks this slice and nests the
//! generated Axum routers under each controller's prefix.

use std::any::TypeId;
use std::sync::Arc;

pub use axum;

use fr_core::Container;

mod pipeline;

pub use pipeline::{
    DefaultValuePipe, ExceptionFilter, Guard, Interceptor, Middleware, Next, ParseIntPipe, Pipe,
    PipeError, RequestCtx, RoutePipeline,
};

/// HTTP verb for a route. Informational — routing is typed at build time.
pub type Method = &'static str;

/// A single generated route factory.
pub struct RouteSpec {
    pub method: Method,
    pub path: &'static str,
    /// Builds a full single-route Axum router for the owning controller.
    pub build: fn(&Container) -> axum::Router,
    /// Comma-separated list of fully-qualified type names for request body extractors,
    /// e.g. `"crate::auth::dto::RegisterDto,crate::some::OtherDto"`. Used by
    /// ferrite-swagger to auto-populate OpenAPI `requestBody` entries. Empty string
    /// when this route has no JSON body parameters.
    pub request_body_types: &'static str,
    /// Comma-separated list of fully-qualified type names for guards on this route,
    /// e.g. `"ferrite_auth_jwt::AuthGuard"`. Used by ferrite-swagger to auto-mark
    /// routes as requiring security (Bearer) when any guard is declared.
    pub guard_types: &'static str,
}

/// The set of routes belonging to one controller, registered globally.
pub struct ControllerRoutes {
    pub controller: TypeId,
    pub prefix: &'static str,
    pub routes: &'static [RouteSpec],
}

inventory::collect!(ControllerRoutes);

/// Iterate every registered controller route set.
pub fn all_controller_routes() -> Vec<&'static ControllerRoutes> {
    inventory::iter::<ControllerRoutes>().collect()
}

/// Common HTTP error type, convertible to an Axum response.
#[derive(Debug, Clone, thiserror::Error)]
#[error("[{status}] {message}")]
pub struct HttpError {
    pub status: axum::http::StatusCode,
    pub message: String,
}

impl HttpError {
    pub fn new(status: axum::http::StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn status(&self) -> axum::http::StatusCode {
        self.status
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(axum::http::StatusCode::BAD_REQUEST, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(axum::http::StatusCode::UNAUTHORIZED, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(axum::http::StatusCode::FORBIDDEN, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(axum::http::StatusCode::NOT_FOUND, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(axum::http::StatusCode::CONFLICT, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

impl axum::response::IntoResponse for HttpError {
    fn into_response(self) -> axum::response::Response {
        let body = serde_json::json!({
            "statusCode": self.status.as_u16(),
            "message": self.message,
        });
        (self.status, axum::Json(body)).into_response()
    }
}

/// Handy alias for handler return types.
pub type HttpResult<T> = Result<T, HttpError>;

/// Extractors re-exported with Nest-flavored names.
pub mod extract {
    pub use axum::extract::{Path, Query, State};
    pub use axum::Json;
}

/// Convenience alias used by generated code.
pub type Shared<T> = Arc<T>;

/// Json extractor re-export.
pub use extract::Json;
