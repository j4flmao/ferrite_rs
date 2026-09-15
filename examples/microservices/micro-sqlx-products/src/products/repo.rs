use crate::products::dto::Product;
use dashmap::DashMap;
use ferrite_cache_redis::CacheService;
use ferrite_macros::{inject, injectable};
use tokio::sync::Mutex;

#[injectable]
pub struct ProductsRepo {
    inner: DashMap<String, Product>,
    ordered: Mutex<Vec<String>>,
    cache: CacheService,
}

impl ProductsRepo {
    #[inject]
    pub fn new(cache: CacheService) -> Self {
        Self {
            inner: DashMap::new().into(),
            ordered: Mutex::new(Vec::new()).into(),
            cache,
        }
    }

    fn cache_key(id: &str) -> String {
        format!("product:{id}")
    }

    pub async fn insert(&self, product: Product) {
        let key = Self::cache_key(&product.id);
        self.ordered.lock().await.push(product.id.clone());
        self.inner.insert(product.id.clone(), product.clone());
        let _ = self.cache.set_with_ttl(&key, &product, 300).await;
    }

    pub async fn get(&self, id: &str) -> Option<Product> {
        let key = Self::cache_key(id);
        if let Ok(Some(cached)) = self.cache.get::<Product>(&key).await {
            return Some(cached);
        }
        let from_mem = self.inner.get(id).map(|x| x.clone());
        if let Some(p) = &from_mem {
            let _ = self.cache.set_with_ttl(&key, p, 300).await;
        }
        from_mem
    }

    pub async fn list(&self, limit: usize, offset: usize) -> (usize, Vec<Product>) {
        let guard = self.ordered.lock().await;
        let total = guard.len();
        let ids: Vec<String> = guard.iter().skip(offset).take(limit).cloned().collect();
        drop(guard);
        let mut items = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(p) = self.inner.get(&id).map(|x| x.clone()) {
                items.push(p);
            }
        }
        (total, items)
    }

    pub async fn update_stock(&self, product_id: &str, delta: i64) -> Option<(i64, i64, Product)> {
        let mut g = self.inner.get_mut(product_id)?;
        let old_stock = g.stock;
        let new_stock = (old_stock + delta).max(0);
        g.stock = new_stock;
        g.updated_at = chrono::Utc::now().to_rfc3339();
        let product = g.clone();
        drop(g);
        let key = Self::cache_key(product_id);
        let _ = self.cache.set_with_ttl(&key, &product, 300).await;
        Some((old_stock, new_stock, product))
    }
}
