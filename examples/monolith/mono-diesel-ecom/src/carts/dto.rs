use ferrite_framework::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, Serialize, Validate, ToSchema)]
pub struct AddItemDto {
    #[schema(example = 1)]
    pub product_id: i64,

    #[validate(range(min = 1, max = 999, message = "quantity must be between 1 and 999"))]
    #[schema(example = 2, minimum = 1, maximum = 999)]
    pub quantity: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Validate, ToSchema)]
pub struct UpdateQtyDto {
    #[validate(range(min = 1, max = 999, message = "quantity must be between 1 and 999"))]
    #[schema(example = 3, minimum = 1, maximum = 999)]
    pub quantity: i64,
}
