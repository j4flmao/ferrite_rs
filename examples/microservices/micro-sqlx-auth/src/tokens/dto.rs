use ferrite_framework::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct RefreshTokenDto {
    #[validate(not_empty)]
    #[schema(example = "a1b2c3d4e5f6...refresh-token-hex...")]
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct RevokeTokenDto {
    #[schema(example = 86400)]
    #[serde(default = "default_ttl")]
    pub ttl_seconds: Option<u64>,
}

fn default_ttl() -> Option<u64> {
    Some(86400)
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TokenStatusDto {
    #[schema(example = "revoked")]
    pub status: String,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub at: String,
    #[schema(example = 86400)]
    pub ttl_seconds: u64,
}
