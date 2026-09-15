//! Validation framework: the `Validate` trait, structured errors, the built-in
//! `ValidationPipe`, and a validating `Json<T>` body extractor.
//!
//! DTOs opt in with `#[derive(Validate)]` (re-exported by the `ferrite-framework`
//! facade):
//!
//! ```text
//! use ferrite_framework::Validate;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize, Validate)]
//! pub struct CreateUserDto {
//!     #[validate(email)]
//!     pub email: String,
//!     #[validate(length(min = 8))]
//!     pub password: String,
//!     #[validate(range(min = 13, max = 150))]
//!     pub age: u8,
//! }
//! ```
//!
//! `Json<T>` body extraction auto-runs the generated validator and fails with
//! a structured `422 Unprocessable Entity` response on invalid input.

mod errors;
mod json;
mod pipe;

pub use errors::{FieldError, ValidationErrors};
pub use json::Json;
pub use pipe::ValidationPipe;

/// Implemented by the `#[derive(Validate)]` macro generated code.
pub trait Validate {
    /// Validate this value, returning the collected field errors.
    fn validate(&self) -> Result<(), ValidationErrors>;
}
