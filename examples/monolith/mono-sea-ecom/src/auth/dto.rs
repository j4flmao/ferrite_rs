use ferrite_framework::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct RegisterDto {
    #[validate(email)]
    #[schema(example = "alice@example.com")]
    pub email: String,
    #[validate(length(min = 6))]
    #[schema(example = "secret123", min_length = 6)]
    pub password: String,
    #[validate(not_empty)]
    #[schema(example = "Alice")]
    pub name: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct LoginDto {
    #[validate(email)]
    #[schema(example = "alice@example.com")]
    pub email: String,
    #[validate(not_empty)]
    #[schema(example = "secret123")]
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0")]
    pub token: String,
    #[schema(example = "Bearer")]
    pub token_type: &'static str,
    #[schema(example = 86400)]
    pub expires_in: usize,
    #[schema(example = 1)]
    pub user_id: i64,
    #[schema(example = "alice@example.com")]
    pub email: String,
}
