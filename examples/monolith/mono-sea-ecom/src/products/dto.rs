use ferrite_framework::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateProductDto {
    #[validate(not_empty)]
    #[schema(example = "Mechanical Keyboard")]
    pub name: String,
    #[validate(not_empty)]
    #[schema(example = "mechanical-keyboard")]
    pub slug: String,
    #[schema(example = "A premium mechanical keyboard with RGB backlight.")]
    pub description: String,
    #[schema(example = 12999)]
    pub price_cents: i64,
    #[schema(example = 50)]
    pub stock: i64,
    #[schema(example = 1)]
    pub category_id: Option<i64>,
    pub images: Vec<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateProductDto {
    #[schema(example = "Mechanical Keyboard V2")]
    pub name: Option<String>,
    #[schema(example = "mechanical-keyboard-v2")]
    pub slug: Option<String>,
    #[schema(example = "Updated description.")]
    pub description: Option<String>,
    #[schema(example = 13999)]
    pub price_cents: Option<i64>,
    #[schema(example = 25)]
    pub stock: Option<i64>,
    #[schema(example = 1)]
    pub category_id: Option<i64>,
    pub images: Option<Vec<String>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProductResponse {
    #[schema(example = 1)]
    pub id: i64,
    #[schema(example = "Mechanical Keyboard")]
    pub name: String,
    #[schema(example = "mechanical-keyboard")]
    pub slug: String,
    #[schema(example = "A premium mechanical keyboard with RGB backlight.")]
    pub description: String,
    #[schema(example = 12999)]
    pub price_cents: i64,
    #[schema(example = 50)]
    pub stock: i64,
    #[schema(example = 1)]
    pub category_id: Option<i64>,
    pub images: Vec<String>,
    #[schema(example = 1720000000)]
    pub created_at: i64,
}

impl From<crate::products::models::Product> for ProductResponse {
    fn from(p: crate::products::models::Product) -> Self {
        Self {
            id: p.id,
            name: p.name,
            slug: p.slug,
            description: p.description,
            price_cents: p.price_cents,
            stock: p.stock,
            category_id: p.category_id,
            images: p.images,
            created_at: p.created_at.timestamp(),
        }
    }
}
