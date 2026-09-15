//! A validating `Json<T>` extractor/response.
//!
//! As a request extractor it runs the DTO's `#[validate]` rules after
//! deserialization (mirroring Nest's global `ValidationPipe`): malformed JSON
//! yields `400`, failing validation yields a structured `422`. As a response
//! it behaves exactly like `axum::Json` (serialization only).

use axum::extract::{FromRequest, Request};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json as AxumJson;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use crate::{Validate, ValidationErrors};

/// JSON body extractor with automatic `#[validate]` checks; also a response
/// wrapper. See the module docs for details.
pub struct Json<T>(pub T);

impl<T, S> FromRequest<S> for Json<T>
where
    T: DeserializeOwned + Validate + Send + 'static,
    S: Send + Sync,
{
    type Rejection = (StatusCode, AxumJson<Value>);

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let invalid = || {
            (
                StatusCode::BAD_REQUEST,
                AxumJson(json!({ "statusCode": 400, "message": "invalid JSON body" })),
            )
        };

        let AxumJson(value) = AxumJson::<T>::from_request(req, state)
            .await
            .map_err(|_| invalid())?;

        value.validate().map_err(|errors: ValidationErrors| {
            let mut body = errors.to_value();
            if let Value::Object(map) = &mut body {
                map.insert("statusCode".to_string(), json!(422));
                map.insert("message".to_string(), json!("validation failed"));
            }
            (StatusCode::UNPROCESSABLE_ENTITY, AxumJson(body))
        })?;

        Ok(Json(value))
    }
}

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        AxumJson(self.0).into_response()
    }
}
