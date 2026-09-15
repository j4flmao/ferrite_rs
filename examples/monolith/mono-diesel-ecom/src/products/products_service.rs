use std::sync::atomic::{AtomicI64, Ordering};

use chrono::Utc;
use dashmap::DashMap;
use ferrite_framework::{inject, injectable};

use super::models::Product;

static NEXT_ID: AtomicI64 = AtomicI64::new(1);

#[injectable]
pub struct ProductsService {
    store: DashMap<i64, Product>,
}

impl Default for ProductsService {
    fn default() -> Self {
        Self::new()
    }
}

impl ProductsService {
    #[inject]
    pub fn new() -> Self {
        let store = DashMap::new();
        let seed = [
            (
                "ThinkPad X1 Carbon",
                "thinkpad-x1",
                "14\" ultrabook, 16GB RAM, 512GB SSD.",
                1899_00,
                12,
                Some(2),
            ),
            (
                "iPhone 16 Pro",
                "iphone-16-pro",
                "6.3\" OLED, A18 Pro, 256GB.",
                1199_00,
                34,
                Some(3),
            ),
            (
                "Organic Cotton Tee",
                "cotton-tee",
                "100% organic cotton, unisex fit.",
                29_90,
                200,
                Some(4),
            ),
        ];
        for (name, slug, desc, price, stock, cat) in seed {
            let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
            store.insert(
                id,
                Product {
                    id,
                    name: name.into(),
                    slug: slug.into(),
                    description: desc.into(),
                    price_cents: price,
                    stock,
                    category_id: cat,
                    images: vec![format!("https://picsum.photos/seed/{slug}/600/400")],
                    created_at: Utc::now(),
                },
            );
        }
        Self {
            store: store.into(),
        }
    }

    pub async fn find_all(&self) -> Vec<Product> {
        self.store.iter().map(|r| r.value().clone()).collect()
    }

    pub async fn find_one(&self, id: i64) -> Option<Product> {
        self.store.get(&id).map(|r| r.value().clone())
    }

    pub async fn find_by_category(&self, category_id: i64) -> Vec<Product> {
        self.store
            .iter()
            .filter(|r| r.value().category_id == Some(category_id))
            .map(|r| r.value().clone())
            .collect()
    }

    pub async fn create(
        &self,
        name: String,
        slug: String,
        description: String,
        price_cents: i64,
        stock: i64,
        category_id: Option<i64>,
        images: Vec<String>,
    ) -> Product {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let p = Product {
            id,
            name,
            slug,
            description,
            price_cents,
            stock,
            category_id,
            images,
            created_at: Utc::now(),
        };
        self.store.insert(id, p.clone());
        p
    }

    pub async fn update(
        &self,
        id: i64,
        name: Option<String>,
        slug: Option<String>,
        description: Option<String>,
        price_cents: Option<i64>,
        stock: Option<i64>,
        category_id: Option<Option<i64>>,
        images: Option<Vec<String>>,
    ) -> Option<Product> {
        let mut p = self.store.get_mut(&id)?;
        if let Some(v) = name {
            p.name = v;
        }
        if let Some(v) = slug {
            p.slug = v;
        }
        if let Some(v) = description {
            p.description = v;
        }
        if let Some(v) = price_cents {
            p.price_cents = v;
        }
        if let Some(v) = stock {
            p.stock = v;
        }
        if let Some(v) = category_id {
            p.category_id = v;
        }
        if let Some(v) = images {
            p.images = v;
        }
        Some(p.value().clone())
    }

    pub async fn decr_stock(&self, id: i64, qty: i64) -> Option<Product> {
        let mut p = self.store.get_mut(&id)?;
        p.stock = (p.stock - qty).max(0);
        Some(p.value().clone())
    }

    pub async fn delete(&self, id: i64) -> bool {
        self.store.remove(&id).is_some()
    }
}
