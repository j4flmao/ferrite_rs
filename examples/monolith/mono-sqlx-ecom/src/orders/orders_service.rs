use std::sync::atomic::{AtomicI64, Ordering};

use chrono::Utc;
use dashmap::DashMap;
use ferrite_cache_redis::CacheService;
use ferrite_framework::{inject, injectable};
use ferrite_kafka::KafkaServer;

use super::models::{Order, OrderCreatedEvent, OrderItem, OrderStatus};
use crate::carts::{models::CartItem as ServiceCartItem, CartsService};
use crate::products::ProductsService;
use crate::users::UsersService;

static NEXT_ID: AtomicI64 = AtomicI64::new(1);

#[injectable]
pub struct OrdersService {
    store: DashMap<i64, Order>,
    carts: CartsService,
    products: ProductsService,
    users: UsersService,
    cache: CacheService,
    kafka: KafkaServer,
}

impl OrdersService {
    #[inject]
    pub fn new(
        carts: CartsService,
        products: ProductsService,
        users: UsersService,
        cache: CacheService,
        kafka: KafkaServer,
    ) -> Self {
        Self {
            store: DashMap::new().into(),
            carts,
            products,
            users,
            cache,
            kafka,
        }
    }

    pub async fn checkout(
        &self,
        user_id: i64,
        shipping_address: String,
        notes: Option<String>,
    ) -> Result<Order, String> {
        let cart_items: Vec<ServiceCartItem> = self.carts.get_items(user_id).await;
        if cart_items.is_empty() {
            return Err(String::from("cart is empty"));
        }

        let _user = self
            .users
            .find_one(user_id)
            .await
            .ok_or_else(|| String::from("user not found"))?;

        let mut order_items = Vec::with_capacity(cart_items.len());
        let mut total_cents = 0i64;

        for item in &cart_items {
            let product = self
                .products
                .find_one(item.product_id)
                .await
                .ok_or_else(|| format!("product {} not found", item.product_id))?;
            if product.stock < item.quantity {
                return Err(format!(
                    "product {} out of stock (have={}, need={})",
                    product.id, product.stock, item.quantity
                ));
            }
        }

        for item in &cart_items {
            let product = self.products.find_one(item.product_id).await.unwrap();
            let line_total = product.price_cents * item.quantity;
            total_cents += line_total;
            order_items.push(OrderItem {
                order_id: 0,
                product_id: product.id,
                product_name: product.name,
                product_image: product.images.first().cloned().unwrap_or_default(),
                price_cents: product.price_cents,
                quantity: item.quantity,
                line_total_cents: line_total,
            });
            let _ = self.products.decr_stock(product.id, item.quantity).await;
        }

        let order_id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        for oi in order_items.iter_mut() {
            oi.order_id = order_id;
        }
        let now = Utc::now();
        let order = Order {
            id: order_id,
            user_id,
            total_cents,
            status: OrderStatus::Pending,
            shipping_address,
            notes,
            items: order_items,
            created_at: now,
            updated_at: now,
        };

        self.store.insert(order_id, order.clone());

        let cache_key = format!("orders:{user_id}:{order_id}");
        if let Err(e) = self.cache.set_with_ttl(&cache_key, &order, 3600).await {
            eprintln!("[orders] cache set failed: {e}");
        }

        let event = OrderCreatedEvent {
            order_id,
            user_id,
            total_cents,
            items_count: order.items.len(),
            created_at: now,
        };
        if let Err(e) = self
            .kafka
            .produce("orders.created", Some(&user_id.to_string()), &event)
        {
            eprintln!("[orders] kafka produce failed: {e}");
        }

        self.carts.clear(user_id).await;

        Ok(order)
    }

    pub async fn find_all_for_user(&self, user_id: i64, is_admin: bool) -> Vec<Order> {
        self.store
            .iter()
            .filter(|r| is_admin || r.value().user_id == user_id)
            .map(|r| r.value().clone())
            .collect()
    }

    pub async fn find_one_for_user(
        &self,
        user_id: i64,
        order_id: i64,
        is_admin: bool,
    ) -> Option<Order> {
        let order = self.store.get(&order_id)?;
        if !is_admin && order.user_id != user_id {
            return None;
        }
        Some(order.value().clone())
    }
}
