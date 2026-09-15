use crate::orders::dto::{Order, OrderItem};
use dashmap::DashMap;
use ferrite_cache_redis::CacheService;
use ferrite_macros::{inject, injectable};
use tokio::sync::Mutex;

#[injectable]
pub struct OrdersRepo {
    inner: DashMap<String, Order>,
    ordered: Mutex<Vec<String>>,
    cache: CacheService,
}

impl OrdersRepo {
    #[inject]
    pub fn new(cache: CacheService) -> Self {
        Self {
            inner: DashMap::new().into(),
            ordered: Mutex::new(Vec::new()).into(),
            cache,
        }
    }

    fn cache_key(id: &str) -> String {
        format!("order:{id}")
    }

    pub async fn insert(&self, order: Order) {
        let key = Self::cache_key(&order.id);
        self.ordered.lock().await.push(order.id.clone());
        self.inner.insert(order.id.clone(), order.clone());
        let _ = self.cache.set_with_ttl(&key, &order, 300).await;
    }

    pub async fn get(&self, id: &str) -> Option<Order> {
        let key = Self::cache_key(id);
        if let Ok(Some(cached)) = self.cache.get::<Order>(&key).await {
            return Some(cached);
        }
        let from_mem = self.inner.get(id).map(|x| x.clone());
        if let Some(p) = &from_mem {
            let _ = self.cache.set_with_ttl(&key, p, 300).await;
        }
        from_mem
    }

    pub async fn list_by_user(
        &self,
        user_id: i64,
        limit: usize,
        offset: usize,
    ) -> (usize, Vec<Order>) {
        let mut user_orders: Vec<Order> = self
            .inner
            .iter()
            .filter(|x| x.value().user_id == user_id)
            .map(|x| x.value().clone())
            .collect();
        user_orders.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let total = user_orders.len();
        let items: Vec<Order> = user_orders.into_iter().skip(offset).take(limit).collect();
        (total, items)
    }

    pub async fn list_all(&self, limit: usize, offset: usize) -> (usize, Vec<Order>) {
        let guard = self.ordered.lock().await;
        let total = guard.len();
        let ids: Vec<String> = guard
            .iter()
            .rev()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();
        drop(guard);
        let mut items = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(p) = self.inner.get(&id).map(|x| x.clone()) {
                items.push(p);
            }
        }
        (total, items)
    }

    pub async fn update_status(
        &self,
        order_id: &str,
        status: &str,
    ) -> Option<(String, String, Order)> {
        let mut g = self.inner.get_mut(order_id)?;
        let old_status = g.status.clone();
        g.status = status.to_string();
        g.updated_at = chrono::Utc::now().to_rfc3339();
        let order = g.clone();
        drop(g);
        let key = Self::cache_key(order_id);
        let _ = self.cache.set_with_ttl(&key, &order, 300).await;
        Some((old_status, status.to_string(), order))
    }

    pub async fn cancel(&self, order_id: &str) -> Option<Order> {
        let mut g = self.inner.get_mut(order_id)?;
        g.status = "cancelled".to_string();
        g.updated_at = chrono::Utc::now().to_rfc3339();
        let order = g.clone();
        drop(g);
        let key = Self::cache_key(order_id);
        let _ = self.cache.set_with_ttl(&key, &order, 300).await;
        Some(order)
    }
}

#[injectable]
pub struct OrderItemsRepo {
    inner: DashMap<String, OrderItem>,
    by_order: DashMap<String, Vec<String>>,
}

impl OrderItemsRepo {
    #[inject]
    pub fn new() -> Self {
        Self {
            inner: DashMap::new().into(),
            by_order: DashMap::new().into(),
        }
    }

    pub async fn insert(&self, item: OrderItem) {
        self.by_order
            .entry(item.order_id.clone())
            .or_default()
            .push(item.id.clone());
        self.inner.insert(item.id.clone(), item);
    }

    pub async fn list_by_order(&self, order_id: &str) -> Vec<OrderItem> {
        let ids = self
            .by_order
            .get(order_id)
            .map(|v| v.value().clone())
            .unwrap_or_default();
        let mut items = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(item) = self.inner.get(&id).map(|x| x.value().clone()) {
                items.push(item);
            }
        }
        items
    }
}
