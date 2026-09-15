use ferrite_framework::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, Serialize, Validate, ToSchema)]
pub struct CheckoutDto {
    #[validate(length(min = 3, max = 400))]
    #[schema(
        example = "123 Main St, District 1, HCM City",
        min_length = 3,
        max_length = 400
    )]
    pub shipping_address: String,

    #[schema(example = "Please ring the doorbell twice.")]
    pub notes: Option<String>,
}
