use ferrite_framework::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateCategoryDto {
    #[validate(not_empty)]
    #[schema(example = "Electronics")]
    pub name: String,
    #[validate(not_empty)]
    #[schema(example = "electronics")]
    pub slug: String,
    #[schema(example = "All kinds of consumer electronics.")]
    pub description: Option<String>,
    #[schema(example = "null")]
    pub parent_id: Option<i64>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateCategoryDto {
    #[schema(example = "Electronics updated")]
    pub name: Option<String>,
    #[schema(example = "electronics-v2")]
    pub slug: Option<String>,
    #[schema(example = "All kinds of consumer electronics updated.")]
    pub description: Option<String>,
    #[schema(example = "null")]
    pub parent_id: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CategoryResponse {
    #[schema(example = 1)]
    pub id: i64,
    #[schema(example = "Electronics")]
    pub name: String,
    #[schema(example = "electronics")]
    pub slug: String,
    #[schema(example = "All kinds of consumer electronics.")]
    pub description: Option<String>,
    #[schema(example = "null")]
    pub parent_id: Option<i64>,
    #[schema(example = 1720000000)]
    pub created_at: i64,
}

impl From<crate::categories::models::Category> for CategoryResponse {
    fn from(c: crate::categories::models::Category) -> Self {
        Self {
            id: c.id,
            name: c.name,
            slug: c.slug,
            description: c.description,
            parent_id: c.parent_id,
            created_at: c.created_at.timestamp(),
        }
    }
}
