use std::sync::atomic::{AtomicI64, Ordering};

use chrono::Utc;
use dashmap::DashMap;
use ferrite_framework::{inject, injectable};

use super::models::Category;

static NEXT_ID: AtomicI64 = AtomicI64::new(1);

#[injectable]
pub struct CategoriesService {
    store: DashMap<i64, Category>,
}

impl Default for CategoriesService {
    fn default() -> Self {
        Self::new()
    }
}

impl CategoriesService {
    #[inject]
    pub fn new() -> Self {
        let store = DashMap::new();
        let seed: [(i64, &str, &str, Option<i64>); 4] = [
            (1, "Electronics", "electronics", None),
            (2, "Laptops", "laptops", Some(1)),
            (3, "Smartphones", "smartphones", Some(1)),
            (4, "Clothing", "clothing", None),
        ];
        NEXT_ID.fetch_add(seed.len() as i64, Ordering::SeqCst);
        for (id, name, slug, parent_id) in seed {
            store.insert(
                id,
                Category {
                    id,
                    name: name.into(),
                    slug: slug.into(),
                    description: None,
                    parent_id,
                    created_at: Utc::now(),
                },
            );
        }
        Self {
            store: store.into(),
        }
    }

    pub async fn find_all(&self) -> Vec<Category> {
        self.store.iter().map(|r| r.value().clone()).collect()
    }

    pub async fn find_one(&self, id: i64) -> Option<Category> {
        self.store.get(&id).map(|r| r.value().clone())
    }

    pub async fn find_by_slug(&self, slug: &str) -> Option<Category> {
        self.store
            .iter()
            .find(|r| r.value().slug == slug)
            .map(|r| r.value().clone())
    }

    pub async fn children_of(&self, parent_id: i64) -> Vec<Category> {
        self.store
            .iter()
            .filter(|r| r.value().parent_id == Some(parent_id))
            .map(|r| r.value().clone())
            .collect()
    }

    pub async fn create(
        &self,
        name: String,
        slug: String,
        description: Option<String>,
        parent_id: Option<i64>,
    ) -> Result<Category, String> {
        if self.find_by_slug(&slug).await.is_some() {
            return Err(format!("slug '{slug}' already exists"));
        }
        if let Some(pid) = parent_id {
            if self.find_one(pid).await.is_none() {
                return Err(format!("parent category {pid} not found"));
            }
        }
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let cat = Category {
            id,
            name,
            slug,
            description,
            parent_id,
            created_at: Utc::now(),
        };
        self.store.insert(id, cat.clone());
        Ok(cat)
    }

    pub async fn update(
        &self,
        id: i64,
        name: Option<String>,
        slug: Option<String>,
        description: Option<Option<String>>,
        parent_id: Option<Option<i64>>,
    ) -> Result<Category, String> {
        if let Some(ref s) = slug {
            if let Some(existing) = self.find_by_slug(s).await {
                if existing.id != id {
                    return Err(format!("slug '{s}' already exists"));
                }
            }
        }
        if let Some(Some(pid)) = parent_id {
            if pid == id {
                return Err("cannot set self as parent".into());
            }
            if self.find_one(pid).await.is_none() {
                return Err(format!("parent category {pid} not found"));
            }
        }
        let mut cat = self
            .store
            .get_mut(&id)
            .ok_or_else(|| format!("category {id} not found"))?;
        if let Some(v) = name {
            cat.name = v;
        }
        if let Some(v) = slug {
            cat.slug = v;
        }
        if let Some(v) = description {
            cat.description = v;
        }
        if let Some(v) = parent_id {
            cat.parent_id = v;
        }
        Ok(cat.value().clone())
    }

    pub async fn delete(&self, id: i64) -> Result<bool, String> {
        let children = self.children_of(id).await;
        if !children.is_empty() {
            return Err("cannot delete category with children".into());
        }
        Ok(self.store.remove(&id).is_some())
    }
}
