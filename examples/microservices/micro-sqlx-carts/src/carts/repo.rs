use crate::carts::dto::{Cart, CartItem};
use dashmap::DashMap;
use ferrite_cache_redis::CacheService;
use ferrite_macros::{inject, injectable};
use tokio::sync::Mutex;

const GUEST_CART_TTL: u64 = 86400;
const CART_TTL: u64 = 3600;

#[injectable]
pub struct CartsRepo {
    inner: DashMap<String, Cart>,
    ordered: Mutex<Vec<String>>,
    by_user: DashMap<i64, String>,
    by_session: DashMap<String, String>,
    cache: CacheService,
}

impl CartsRepo {
    #[inject]
    pub fn new(cache: CacheService) -> Self {
        Self {
            inner: DashMap::new().into(),
            ordered: Mutex::new(Vec::new()).into(),
            by_user: DashMap::new().into(),
            by_session: DashMap::new().into(),
            cache,
        }
    }

    fn cart_key(id: &str) -> String {
        format!("cart:{id}")
    }

    pub fn recalculate_totals(cart: &mut Cart) {
        let mut total = 0.0f64;
        let mut count = 0i64;
        for item in &cart.items {
            total += item.subtotal;
            count += item.quantity;
        }
        cart.total_amount = (total * 100.0).round() / 100.0;
        cart.item_count = count;
        cart.updated_at = chrono::Utc::now().to_rfc3339();
    }

    async fn cache_cart(&self, cart: &Cart) {
        let key = Self::cart_key(&cart.id);
        let ttl = if cart.user_id == -1 {
            GUEST_CART_TTL
        } else {
            CART_TTL
        };
        let _ = self.cache.set_with_ttl(&key, cart, ttl).await;
    }

    async fn invalidate_cart_cache(&self, id: &str) {
        let key = Self::cart_key(id);
        let _ = self.cache.delete(&key).await;
    }

    pub async fn create(&self, cart: Cart) {
        let id = cart.id.clone();
        let user_id = cart.user_id;
        let session_id = cart.session_id.clone();

        self.ordered.lock().await.push(id.clone());
        if user_id != -1 {
            self.by_user.insert(user_id, id.clone());
        }
        if !session_id.is_empty() {
            self.by_session.insert(session_id, id.clone());
        }
        self.inner.insert(id.clone(), cart.clone());
        self.cache_cart(&cart).await;
    }

    pub async fn get(&self, id: &str) -> Option<Cart> {
        let key = Self::cart_key(id);
        if let Ok(Some(cached)) = self.cache.get::<Cart>(&key).await {
            return Some(cached);
        }
        let from_mem = self.inner.get(id).map(|x| x.clone());
        if let Some(c) = &from_mem {
            self.cache_cart(c).await;
        }
        from_mem
    }

    pub async fn get_by_user_or_session(
        &self,
        user_id: Option<i64>,
        session_id: Option<&str>,
    ) -> Option<Cart> {
        if let Some(uid) = user_id {
            if uid != -1 {
                if let Some(cart_id) = self.by_user.get(&uid).map(|x| x.clone()) {
                    return self.get(&cart_id).await;
                }
            }
        }
        if let Some(sid) = session_id {
            if !sid.is_empty() {
                if let Some(cart_id) = self.by_session.get(sid).map(|x| x.clone()) {
                    return self.get(&cart_id).await;
                }
            }
        }
        None
    }

    pub async fn add_item(&self, cart_id: &str, item: CartItem) -> Option<Cart> {
        let mut cart = self.inner.get_mut(cart_id)?;
        if let Some(existing) = cart
            .items
            .iter_mut()
            .find(|i| i.product_id == item.product_id)
        {
            existing.quantity += item.quantity;
            existing.subtotal =
                (existing.unit_price * existing.quantity as f64 * 100.0).round() / 100.0;
        } else {
            cart.items.push(item);
        }
        Self::recalculate_totals(&mut cart);
        let updated = cart.clone();
        drop(cart);
        self.invalidate_cart_cache(cart_id).await;
        self.cache_cart(&updated).await;
        Some(updated)
    }

    pub async fn update_item(
        &self,
        cart_id: &str,
        item_id: &str,
        quantity: i64,
    ) -> Option<(Cart, i64, i64)> {
        let mut cart = self.inner.get_mut(cart_id)?;
        let item = cart.items.iter_mut().find(|i| i.id == item_id)?;
        let old_qty = item.quantity;
        item.quantity = quantity.max(1);
        item.subtotal = (item.unit_price * item.quantity as f64 * 100.0).round() / 100.0;
        let new_qty = item.quantity;
        Self::recalculate_totals(&mut cart);
        let updated = cart.clone();
        drop(cart);
        self.invalidate_cart_cache(cart_id).await;
        self.cache_cart(&updated).await;
        Some((updated, old_qty, new_qty))
    }

    pub async fn remove_item(&self, cart_id: &str, item_id: &str) -> Option<Cart> {
        let mut cart = self.inner.get_mut(cart_id)?;
        let before = cart.items.len();
        cart.items.retain(|i| i.id != item_id);
        if cart.items.len() == before {
            return None;
        }
        Self::recalculate_totals(&mut cart);
        let updated = cart.clone();
        drop(cart);
        self.invalidate_cart_cache(cart_id).await;
        self.cache_cart(&updated).await;
        Some(updated)
    }

    pub async fn clear(&self, cart_id: &str) -> Option<Cart> {
        let mut cart = self.inner.get_mut(cart_id)?;
        cart.items.clear();
        Self::recalculate_totals(&mut cart);
        let updated = cart.clone();
        drop(cart);
        self.invalidate_cart_cache(cart_id).await;
        self.cache_cart(&updated).await;
        Some(updated)
    }

    pub async fn convert(&self, cart_id: &str, user_id: i64) -> Option<Cart> {
        let mut cart = self.inner.get_mut(cart_id)?;
        let old_session_id = cart.session_id.clone();
        cart.user_id = user_id;
        cart.session_id = String::new();
        cart.status = "active".into();
        cart.expires_at = None;
        Self::recalculate_totals(&mut cart);
        let updated = cart.clone();
        drop(cart);

        if !old_session_id.is_empty() {
            self.by_session.remove(&old_session_id);
        }
        self.by_user.insert(user_id, cart_id.to_string());
        self.invalidate_cart_cache(cart_id).await;
        self.cache_cart(&updated).await;
        Some(updated)
    }

    pub async fn list_all(&self, limit: usize, offset: usize) -> (usize, Vec<Cart>) {
        let guard = self.ordered.lock().await;
        let total = guard.len();
        let ids: Vec<String> = guard.iter().skip(offset).take(limit).cloned().collect();
        drop(guard);
        let mut items = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(c) = self.inner.get(&id).map(|x| x.clone()) {
                items.push(c);
            }
        }
        (total, items)
    }
}
