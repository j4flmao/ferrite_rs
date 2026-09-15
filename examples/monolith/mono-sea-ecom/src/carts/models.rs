use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartItem {
    pub user_id: i64,
    pub product_id: i64,
    pub quantity: i64,
    pub price_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartSummary {
    pub items: Vec<CartItemDetail>,
    pub total_cents: i64,
    pub total_items: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartItemDetail {
    pub product_id: i64,
    pub product_name: String,
    pub product_image: String,
    pub quantity: i64,
    pub unit_price_cents: i64,
    pub line_total_cents: i64,
}
