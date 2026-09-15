use ferrite_framework::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CheckoutItemDto {
    #[schema(example = "prod_01ARZ3NDEKTSV4RRFFQ69G5FAV")]
    pub product_id: String,
    #[validate(range(min = 1.0))]
    #[schema(example = 2, minimum = 1)]
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CheckoutDto {
    pub items: Vec<CheckoutItemDto>,
    #[schema(example = "123 Main St, New York, NY 10001", nullable = true)]
    pub shipping_address: Option<String>,
    #[schema(example = "Please leave at front door", nullable = true)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CheckoutSession {
    #[schema(example = "ord_01ARZ3NDEKTSV4RRFFQ69G5FAV")]
    pub order_id: String,
    #[schema(example = "https://pay.example.com/order_ord_01ARZ3NDEKTSV4RRFFQ69G5FAV")]
    pub payment_url: String,
    #[schema(example = "2026-09-14T16:00:00Z")]
    pub expires_at: String,
    #[schema(example = 149.97)]
    pub total_amount: f64,
    #[schema(example = "USD")]
    pub currency: String,
}
