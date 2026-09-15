use crate::checkout::dto::{CheckoutDto, CheckoutSession};
use crate::orders::dto::{CreateOrderDto, OrderItemDto};
use crate::orders::handlers::KafkaOrderPublisher;
use crate::orders::service::OrdersService;
use ferrite_framework::{inject, HttpError};
use ferrite_macros::injectable as injectable_attr;

#[injectable_attr]
pub struct CheckoutService {
    orders_service: OrdersService,
    kafka_publisher: KafkaOrderPublisher,
}

impl CheckoutService {
    #[inject]
    pub fn new(orders_service: OrdersService, kafka_publisher: KafkaOrderPublisher) -> Self {
        Self {
            orders_service,
            kafka_publisher,
        }
    }

    pub async fn create_session(
        &self,
        dto: CheckoutDto,
        user_id: i64,
    ) -> Result<CheckoutSession, HttpError> {
        if dto.items.is_empty() {
            return Err(HttpError::bad_request(
                "checkout must contain at least one item",
            ));
        }
        for item in &dto.items {
            if item.quantity < 1 {
                return Err(HttpError::bad_request("each item quantity must be >= 1"));
            }
        }
        let order_dto = CreateOrderDto {
            items: dto
                .items
                .into_iter()
                .map(|i| OrderItemDto {
                    product_id: i.product_id,
                    quantity: i.quantity,
                })
                .collect(),
            shipping_address: dto.shipping_address,
            notes: dto.notes,
        };
        let order = self.orders_service.create_order(order_dto, user_id).await?;
        let payment_url = format!("https://pay.example.com/order_{}", order.order.id);
        let expires_at = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::minutes(30))
            .map(|d| d.to_rfc3339())
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        let _ = self
            .kafka_publisher
            .publish_checkout_completed(&order.order.id, &payment_url, &expires_at)
            .await;
        Ok(CheckoutSession {
            order_id: order.order.id,
            payment_url,
            expires_at,
            total_amount: order.order.total_amount,
            currency: order.order.currency,
        })
    }
}
