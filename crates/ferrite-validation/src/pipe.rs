//! The `ValidationPipe` drives `#[validate]` DTOs through the request pipeline.

use axum::response::IntoResponse;
use ferrite_http::{Pipe, PipeError};

use crate::Validate;

/// Validates a DTO before it reaches the handler. Fails with a structured
/// `422` response carried inside [`PipeError::Response`].
pub struct ValidationPipe;

impl<T> Pipe<T> for ValidationPipe
where
    T: Validate + Send + Sync + 'static,
{
    type Output = T;

    fn transform(&self, value: T) -> Result<T, PipeError> {
        match value.validate() {
            Ok(()) => Ok(value),
            Err(errors) => Err(PipeError::response(errors.into_response())),
        }
    }
}
