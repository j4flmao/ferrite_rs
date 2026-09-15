use ferrite_framework::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct RegisterDto {
    #[validate(email)]
    #[schema(example = "alice@example.com")]
    pub email: String,

    #[validate(length(min = 6, max = 64))]
    #[schema(example = "secret123", min_length = 6, max_length = 64)]
    pub password: String,

    #[validate(length(min = 2, max = 50))]
    #[schema(example = "Alice", min_length = 2, max_length = 50)]
    pub name: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct LoginDto {
    #[validate(email)]
    #[schema(example = "alice@example.com")]
    pub email: String,

    #[validate(length(min = 6, max = 64))]
    #[schema(example = "secret123", min_length = 6, max_length = 64)]
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    #[schema(example = "eyJhbGciOiJIUzI1NiJ9.xxx.yyy")]
    pub token: String,
    #[schema(example = "Bearer")]
    pub token_type: &'static str,
    #[schema(example = 86400)]
    pub expires_in: usize,
    #[schema(example = 1)]
    pub user_id: i64,
    #[schema(example = "alice@example.com")]
    pub email: String,
    #[schema(example = "Alice")]
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MeResponse {
    #[schema(example = 1)]
    pub id: i64,
    #[schema(example = "alice@example.com")]
    pub email: String,
    #[schema(example = "Alice")]
    pub name: String,
}
