use ferrite_framework::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// ========== Domain entities ==========
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Cart {
    #[schema(example = "cart_01ARZ3NDEKTSV4RRFFQ69G5FAV")]
    pub id: String,
    #[schema(example = "1001")]
    pub user_id: i64,
    #[schema(example = "sess_abc123xyz789")]
    pub session_id: String,
    #[schema(example = "active")]
    pub status: String,
    #[schema(example = "89.97")]
    pub total_amount: f64,
    #[schema(example = "3")]
    pub item_count: i64,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub created_at: String,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub updated_at: String,
    #[schema(example = "2026-09-15T15:30:00Z")]
    pub expires_at: Option<String>,
    pub items: Vec<CartItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CartItem {
    #[schema(example = "item_01ARZ3NDEKTSV4RRFFQ69G5FAV")]
    pub id: String,
    #[schema(example = "cart_01ARZ3NDEKTSV4RRFFQ69G5FAV")]
    pub cart_id: String,
    #[schema(example = "prod_01ARZ3NDEKTSV4RRFFQ69G5FAV")]
    pub product_id: String,
    #[schema(example = "Handcrafted Ceramic Mug")]
    pub product_title: String,
    #[schema(example = "24.99")]
    pub unit_price: f64,
    #[schema(example = "2")]
    pub quantity: i64,
    #[schema(example = "49.98")]
    pub subtotal: f64,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub added_at: String,
}

// ========== HTTP DTOs ==========
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateCartDto {
    #[schema(example = "1001")]
    pub user_id: Option<i64>,
    #[schema(example = "sess_abc123xyz789")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct AddToCartDto {
    #[validate(not_empty)]
    #[schema(example = "prod_01ARZ3NDEKTSV4RRFFQ69G5FAV")]
    pub product_id: String,
    #[validate(not_empty)]
    #[schema(example = "Handcrafted Ceramic Mug")]
    pub product_title: String,
    #[validate(range(min = 0.0))]
    #[schema(example = "24.99", minimum = 0.0)]
    pub unit_price: f64,
    #[validate(range(min = 1.0))]
    #[schema(example = "2", minimum = 1)]
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct UpdateCartItemDto {
    #[validate(range(min = 1.0))]
    #[schema(example = "3", minimum = 1)]
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ConvertCartDto {
    #[schema(example = "1001")]
    pub user_id: i64,
}

// ========== CQRS: Commands ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCartCommand {
    pub id: String,
    pub user_id: i64,
    pub session_id: String,
}
impl ferrite_cqrs::Command for CreateCartCommand {}

impl CreateCartCommand {
    pub fn from_dto(dto: CreateCartDto) -> Self {
        Self {
            id: format!("cart_{}", Uuid::new_v4().simple()),
            user_id: dto.user_id.unwrap_or(-1),
            session_id: dto
                .session_id
                .unwrap_or_else(|| format!("sess_{}", Uuid::new_v4().simple())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddToCartCommand {
    pub cart_id: String,
    pub item_id: String,
    pub product_id: String,
    pub product_title: String,
    pub unit_price: f64,
    pub quantity: i64,
}
impl ferrite_cqrs::Command for AddToCartCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCartItemCommand {
    pub cart_id: String,
    pub item_id: String,
    pub quantity: i64,
}
impl ferrite_cqrs::Command for UpdateCartItemCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveFromCartCommand {
    pub cart_id: String,
    pub item_id: String,
}
impl ferrite_cqrs::Command for RemoveFromCartCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearCartCommand {
    pub cart_id: String,
}
impl ferrite_cqrs::Command for ClearCartCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertCartCommand {
    pub cart_id: String,
    pub user_id: i64,
}
impl ferrite_cqrs::Command for ConvertCartCommand {}

// ========== CQRS: Queries ==========
#[derive(Debug, Clone)]
pub struct GetCartQuery {
    pub cart_id: String,
}
impl ferrite_cqrs::Query for GetCartQuery {
    type Result = Option<Cart>;
}

#[derive(Debug, Clone)]
pub struct GetUserCartQuery {
    pub user_id: i64,
}
impl ferrite_cqrs::Query for GetUserCartQuery {
    type Result = Option<Cart>;
}

#[derive(Debug, Clone)]
pub struct GetSessionCartQuery {
    pub session_id: String,
}
impl ferrite_cqrs::Query for GetSessionCartQuery {
    type Result = Option<Cart>;
}

#[derive(Debug, Clone)]
pub struct ListCartsQuery {
    pub limit: usize,
    pub offset: usize,
}
impl ferrite_cqrs::Query for ListCartsQuery {
    type Result = CartList;
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CartList {
    pub total: usize,
    #[schema(example = "20")]
    pub limit: usize,
    #[schema(example = "0")]
    pub offset: usize,
    pub items: Vec<Cart>,
}

// ========== CQRS: Events (fan-out) ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartCreatedEvent {
    pub cart: Cart,
    pub at: String,
}
impl ferrite_cqrs::Event for CartCreatedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartUpdatedEvent {
    pub cart: Cart,
    pub at: String,
}
impl ferrite_cqrs::Event for CartUpdatedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartItemAddedEvent {
    pub cart_id: String,
    pub item: CartItem,
    pub at: String,
}
impl ferrite_cqrs::Event for CartItemAddedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartItemUpdatedEvent {
    pub cart_id: String,
    pub item_id: String,
    pub old_quantity: i64,
    pub new_quantity: i64,
    pub at: String,
}
impl ferrite_cqrs::Event for CartItemUpdatedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartItemRemovedEvent {
    pub cart_id: String,
    pub item_id: String,
    pub at: String,
}
impl ferrite_cqrs::Event for CartItemRemovedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartClearedEvent {
    pub cart_id: String,
    pub at: String,
}
impl ferrite_cqrs::Event for CartClearedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartConvertedEvent {
    pub cart_id: String,
    pub old_session_id: String,
    pub new_user_id: i64,
    pub at: String,
}
impl ferrite_cqrs::Event for CartConvertedEvent {}
