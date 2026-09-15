use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Paid,
    Shipped,
    Delivered,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: i64,
    pub user_id: i64,
    pub total_cents: i64,
    pub status: OrderStatus,
    pub shipping_address: String,
    pub notes: Option<String>,
    pub items: Vec<OrderItem>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItem {
    pub order_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub product_image: String,
    pub price_cents: i64,
    pub quantity: i64,
    pub line_total_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCreatedEvent {
    pub order_id: i64,
    pub user_id: i64,
    pub total_cents: i64,
    pub items_count: usize,
    pub created_at: DateTime<Utc>,
}
