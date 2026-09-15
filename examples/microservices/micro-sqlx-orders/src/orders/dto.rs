use ferrite_framework::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// ========== Domain entities ==========
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Order {
    #[schema(example = "ord_01ARZ3NDEKTSV4RRFFQ69G5FAV")]
    pub id: String,
    #[schema(example = "1001")]
    pub user_id: i64,
    #[schema(example = json!(["pending", "paid", "shipped", "delivered", "cancelled"]))]
    pub status: String,
    #[schema(example = 149.97)]
    pub total_amount: f64,
    #[schema(example = "USD")]
    pub currency: String,
    #[schema(example = "123 Main St, New York, NY 10001", nullable = true)]
    pub shipping_address: Option<String>,
    #[schema(example = "Please leave at front door", nullable = true)]
    pub notes: Option<String>,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub created_at: String,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderItem {
    #[schema(example = "itm_01ARZ3NDEKTSV4RRFFQ69G5FAV")]
    pub id: String,
    #[schema(example = "ord_01ARZ3NDEKTSV4RRFFQ69G5FAV")]
    pub order_id: String,
    #[schema(example = "prod_01ARZ3NDEKTSV4RRFFQ69G5FAV")]
    pub product_id: String,
    #[schema(example = "Handcrafted Ceramic Mug")]
    pub product_title: String,
    #[schema(example = 24.99)]
    pub unit_price: f64,
    #[schema(example = 3)]
    pub quantity: i64,
    #[schema(example = 74.97)]
    pub subtotal: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderWithItems {
    #[serde(flatten)]
    pub order: Order,
    pub items: Vec<OrderItem>,
}

// ========== HTTP DTOs ==========
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct OrderItemDto {
    #[schema(example = "prod_01ARZ3NDEKTSV4RRFFQ69G5FAV")]
    pub product_id: String,
    #[validate(range(min = 1.0))]
    #[schema(example = 2, minimum = 1)]
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateOrderDto {
    pub items: Vec<OrderItemDto>,
    #[schema(example = "123 Main St, New York, NY 10001", nullable = true)]
    pub shipping_address: Option<String>,
    #[schema(example = "Please leave at front door", nullable = true)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct UpdateOrderStatusDto {
    #[validate(not_empty)]
    #[schema(example = json!(["pending", "paid", "shipped", "delivered", "cancelled"]))]
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CancelOrderDto {
    #[schema(example = "Changed my mind", nullable = true)]
    pub reason: Option<String>,
}

// ========== CQRS: Commands ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderCommand {
    pub id: String,
    pub user_id: i64,
    pub items: Vec<OrderItemDto>,
    pub shipping_address: Option<String>,
    pub notes: Option<String>,
    pub total_amount: f64,
    pub currency: String,
}
impl ferrite_cqrs::Command for CreateOrderCommand {}

impl CreateOrderCommand {
    pub fn from_dto(
        dto: CreateOrderDto,
        user_id: i64,
        item_prices: Vec<(String, String, f64)>,
    ) -> Self {
        let calculated_total: f64 = dto
            .items
            .iter()
            .zip(item_prices.iter())
            .map(|(item, (_, _, price))| price * item.quantity as f64)
            .sum();
        Self {
            id: format!("ord_{}", Uuid::new_v4().simple()),
            user_id,
            items: dto.items,
            shipping_address: dto.shipping_address,
            notes: dto.notes,
            total_amount: calculated_total,
            currency: "USD".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateOrderStatusCommand {
    pub order_id: String,
    pub status: String,
}
impl ferrite_cqrs::Command for UpdateOrderStatusCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelOrderCommand {
    pub order_id: String,
    pub user_id: i64,
    pub reason: Option<String>,
}
impl ferrite_cqrs::Command for CancelOrderCommand {}

// ========== CQRS: Queries ==========
#[derive(Debug, Clone)]
pub struct GetOrderQuery {
    pub order_id: String,
    pub user_id: Option<i64>,
    pub is_admin: bool,
}
impl ferrite_cqrs::Query for GetOrderQuery {
    type Result = Option<OrderWithItems>;
}

#[derive(Debug, Clone)]
pub struct ListOrdersQuery {
    pub user_id: Option<i64>,
    pub is_admin: bool,
    pub limit: usize,
    pub offset: usize,
}
impl ferrite_cqrs::Query for ListOrdersQuery {
    type Result = OrderList;
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderList {
    pub total: usize,
    #[schema(example = 20)]
    pub limit: usize,
    #[schema(example = 0)]
    pub offset: usize,
    pub items: Vec<OrderWithItems>,
}

// ========== CQRS: Events (fan-out) ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCreatedEvent {
    pub order: Order,
    pub items: Vec<OrderItem>,
    pub at: String,
}
impl ferrite_cqrs::Event for OrderCreatedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderStatusChangedEvent {
    pub order_id: String,
    pub old_status: String,
    pub new_status: String,
    pub at: String,
}
impl ferrite_cqrs::Event for OrderStatusChangedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCancelledEvent {
    pub order_id: String,
    pub user_id: i64,
    pub reason: Option<String>,
    pub at: String,
}
impl ferrite_cqrs::Event for OrderCancelledEvent {}

pub fn valid_statuses() -> Vec<&'static str> {
    vec!["pending", "paid", "shipped", "delivered", "cancelled"]
}

pub fn is_valid_status(s: &str) -> bool {
    valid_statuses().contains(&s)
}
