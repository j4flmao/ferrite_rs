//! Structured validation errors, rendered as a Nest-style body.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::Value;

/// A single field-level validation failure.
#[derive(Debug, Clone)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

/// All errors collected while validating a DTO.
#[derive(Debug, Clone, Default)]
pub struct ValidationErrors(pub Vec<FieldError>);

impl ValidationErrors {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Record a failure for `field`.
    pub fn push(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.0.push(FieldError {
            field: field.into(),
            message: message.into(),
        });
    }

    pub fn errors(&self) -> &[FieldError] {
        &self.0
    }

    /// Render as `{ "errors": { field: [message, ...], ... } }`.
    pub fn to_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        for e in &self.0 {
            let entry = map
                .entry(e.field.clone())
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Value::Array(arr) = entry {
                arr.push(Value::String(e.message.clone()));
            }
        }
        serde_json::json!({ "errors": Value::Object(map) })
    }
}

impl IntoResponse for ValidationErrors {
    fn into_response(self) -> Response {
        let body = self.to_value();
        let body = crate::json::Json::<Value>(body).into_response();
        (StatusCode::UNPROCESSABLE_ENTITY, body).into_response()
    }
}
