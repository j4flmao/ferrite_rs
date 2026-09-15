use crate::carts::dto::{Cart, CartItem, CreateCartDto};
use crate::carts::repo::CartsRepo;
use ferrite_macros::{inject, injectable};
use uuid::Uuid;

const GUEST_CART_TTL_SECS: i64 = 86400;

#[injectable]
pub struct CartsService {
    repo: CartsRepo,
}

impl CartsService {
    #[inject]
    pub fn new(repo: CartsRepo) -> Self {
        Self { repo }
    }

    pub fn new_cart_id() -> String {
        format!("cart_{}", Uuid::new_v4().simple())
    }

    pub fn new_item_id() -> String {
        format!("item_{}", Uuid::new_v4().simple())
    }

    pub fn build_cart(dto: CreateCartDto) -> Cart {
        let now = chrono::Utc::now();
        let user_id = dto.user_id.unwrap_or(-1);
        let session_id = dto
            .session_id
            .unwrap_or_else(|| format!("sess_{}", Uuid::new_v4().simple()));

        let expires_at = if user_id == -1 {
            Some((now + chrono::Duration::seconds(GUEST_CART_TTL_SECS)).to_rfc3339())
        } else {
            None
        };

        Cart {
            id: Self::new_cart_id(),
            user_id,
            session_id,
            status: "active".into(),
            total_amount: 0.0,
            item_count: 0,
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
            expires_at,
            items: vec![],
        }
    }

    pub fn build_cart_item(
        cart_id: &str,
        product_id: &str,
        product_title: &str,
        unit_price: f64,
        quantity: i64,
    ) -> CartItem {
        let qty = quantity.max(1);
        let price = unit_price.max(0.0);
        CartItem {
            id: Self::new_item_id(),
            cart_id: cart_id.to_string(),
            product_id: product_id.to_string(),
            product_title: product_title.to_string(),
            unit_price: price,
            quantity: qty,
            subtotal: ((price * qty as f64) * 100.0).round() / 100.0,
            added_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub async fn get_or_create_cart(
        &self,
        cart_id: &str,
        user_id: Option<i64>,
        session_id: Option<&str>,
    ) -> Option<Cart> {
        if let Some(cart) = self.repo.get(cart_id).await {
            return Some(cart);
        }
        if let Some(cart) = self.repo.get_by_user_or_session(user_id, session_id).await {
            return Some(cart);
        }
        None
    }
}
