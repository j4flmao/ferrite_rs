use ferrite_framework::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// ========== Domain entity ==========
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Product {
    #[schema(example = "prod_01ARZ3NDEKTSV4RRFFQ69G5FAV")]
    pub id: String,
    #[schema(example = "Handcrafted Ceramic Mug")]
    pub title: String,
    #[schema(example = "Artisan latte mug, coffee-friendly 350ml.")]
    pub description: String,
    #[schema(example = 24.99)]
    pub price: f64,
    #[schema(example = 120)]
    pub stock: i64,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub created_at: String,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub updated_at: String,
}

// ========== HTTP DTOs ==========
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateProductDto {
    #[validate(not_empty)]
    #[validate(length(min = 2, max = 120))]
    #[schema(example = "Handcrafted Ceramic Mug", min_length = 2, max_length = 120)]
    pub title: String,
    #[validate(length(max = 2000))]
    #[schema(
        example = "Artisan latte mug, coffee-friendly 350ml.",
        min_length = 0,
        max_length = 2000
    )]
    pub description: String,
    #[validate(range(min = 0.0))]
    #[schema(example = 24.99, minimum = 0.0)]
    pub price: f64,
    #[validate(range(min = 0.0))]
    #[schema(example = 120, minimum = 0)]
    pub stock: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct UpdateStockDto {
    #[schema(example = 42)]
    pub delta: i64,
}

// ========== CQRS: Commands ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProductCommand {
    pub id: String,
    pub title: String,
    pub description: String,
    pub price: f64,
    pub stock: i64,
}
impl ferrite_cqrs::Command for CreateProductCommand {}

impl CreateProductCommand {
    pub fn from_dto(dto: CreateProductDto) -> Self {
        Self {
            id: format!("prod_{}", Uuid::new_v4().simple()),
            title: dto.title,
            description: dto.description,
            price: dto.price,
            stock: dto.stock.max(0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProductStockCommand {
    pub product_id: String,
    pub delta: i64,
}
impl ferrite_cqrs::Command for UpdateProductStockCommand {}

// ========== CQRS: Queries ==========
#[derive(Debug, Clone)]
pub struct GetProductQuery {
    pub product_id: String,
}
impl ferrite_cqrs::Query for GetProductQuery {
    type Result = Option<Product>;
}

#[derive(Debug, Clone)]
pub struct ListProductsQuery {
    pub limit: usize,
    pub offset: usize,
}
impl ferrite_cqrs::Query for ListProductsQuery {
    type Result = ProductList;
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProductList {
    pub total: usize,
    #[schema(example = 20)]
    pub limit: usize,
    #[schema(example = 0)]
    pub offset: usize,
    pub items: Vec<Product>,
}

// ========== CQRS: Events (fan-out) ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductCreatedEvent {
    pub product: Product,
    pub at: String,
}
impl ferrite_cqrs::Event for ProductCreatedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockUpdatedEvent {
    pub product_id: String,
    pub old_stock: i64,
    pub new_stock: i64,
    pub delta: i64,
    pub at: String,
}
impl ferrite_cqrs::Event for StockUpdatedEvent {}
