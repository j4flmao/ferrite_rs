use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProfileDto {
    #[schema(example = "1001")]
    pub id: i64,
    #[schema(example = "Alice Johnson")]
    pub name: String,
    #[schema(example = "https://example.com/avatars/alice.png", nullable = true)]
    pub avatar_url: Option<String>,
    #[schema(
        example = "Full-stack engineer, open source enthusiast.",
        nullable = true
    )]
    pub bio: Option<String>,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProfileListDto {
    pub total: usize,
    #[schema(example = 20)]
    pub limit: usize,
    #[schema(example = 0)]
    pub offset: usize,
    pub items: Vec<ProfileDto>,
}
