use crate::users::dto::{User, UserInternal};
use dashmap::DashMap;
use ferrite_cache_redis::CacheService;
use ferrite_macros::{inject, injectable};
use tokio::sync::Mutex;

#[injectable]
pub struct UsersRepo {
    inner: DashMap<i64, UserInternal>,
    by_email: DashMap<String, i64>,
    ordered: Mutex<Vec<i64>>,
    cache: CacheService,
}

impl UsersRepo {
    #[inject]
    pub fn new(cache: CacheService) -> Self {
        Self {
            inner: DashMap::new().into(),
            by_email: DashMap::new().into(),
            ordered: Mutex::new(Vec::new()).into(),
            cache,
        }
    }

    fn cache_key(id: i64) -> String {
        format!("user:{id}")
    }

    pub async fn insert(&self, internal: UserInternal) -> User {
        let key = Self::cache_key(internal.id);
        self.ordered.lock().await.push(internal.id);
        self.by_email.insert(internal.email.clone(), internal.id);
        let public = internal.to_public();
        self.inner.insert(internal.id, internal);
        let _ = self.cache.set_with_ttl(&key, &public, 300).await;
        public
    }

    pub async fn get(&self, id: i64) -> Option<User> {
        let key = Self::cache_key(id);
        if let Ok(Some(cached)) = self.cache.get::<User>(&key).await {
            return Some(cached);
        }
        let from_mem = self.inner.get(&id).map(|x| x.to_public());
        if let Some(p) = &from_mem {
            let _ = self.cache.set_with_ttl(&key, p, 300).await;
        }
        from_mem
    }

    pub async fn get_internal(&self, id: i64) -> Option<UserInternal> {
        self.inner.get(&id).map(|x| x.clone())
    }

    pub async fn list(&self, limit: usize, offset: usize) -> (usize, Vec<User>) {
        let guard = self.ordered.lock().await;
        let total = guard.len();
        let ids: Vec<i64> = guard.iter().skip(offset).take(limit).cloned().collect();
        drop(guard);
        let mut items = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(p) = self.inner.get(&id).map(|x| x.to_public()) {
                items.push(p);
            }
        }
        (total, items)
    }

    pub async fn update(
        &self,
        user_id: i64,
        name: Option<String>,
        avatar_url: Option<String>,
        bio: Option<String>,
    ) -> Option<(Vec<String>, User)> {
        let mut g = self.inner.get_mut(&user_id)?;
        let mut changes: Vec<String> = Vec::new();
        if let Some(n) = name {
            if g.name != n {
                g.name = n;
                changes.push("name".into());
            }
        }
        if let Some(a) = avatar_url {
            if g.avatar_url.as_deref() != Some(&a) {
                g.avatar_url = Some(a);
                changes.push("avatar_url".into());
            }
        }
        if let Some(b) = bio {
            if g.bio.as_deref() != Some(&b) {
                g.bio = Some(b);
                changes.push("bio".into());
            }
        }
        g.updated_at = chrono::Utc::now().to_rfc3339();
        let public = g.to_public();
        drop(g);
        let key = Self::cache_key(user_id);
        let _ = self.cache.set_with_ttl(&key, &public, 300).await;
        Some((changes, public))
    }

    pub async fn delete(&self, user_id: i64) -> Option<UserInternal> {
        let (_, removed) = self.inner.remove(&user_id)?;
        self.by_email.remove(&removed.email);
        let mut guard = self.ordered.lock().await;
        guard.retain(|x| *x != user_id);
        drop(guard);
        let key = Self::cache_key(user_id);
        let _ = self.cache.delete(&key).await;
        Some(removed)
    }

    pub fn find_by_email(&self, email: &str) -> Option<UserInternal> {
        let key = email.to_lowercase();
        let id = self.by_email.get(&key)?;
        self.inner.get(id.value()).map(|x| x.clone())
    }

    pub fn email_exists(&self, email: &str) -> bool {
        self.by_email.contains_key(&email.to_lowercase())
    }

    pub async fn change_password(&self, user_id: i64, new_hash: &str) -> Option<()> {
        let mut g = self.inner.get_mut(&user_id)?;
        g.password_hash = new_hash.to_string();
        g.updated_at = chrono::Utc::now().to_rfc3339();
        let public = g.to_public();
        drop(g);
        let key = Self::cache_key(user_id);
        let _ = self.cache.set_with_ttl(&key, &public, 300).await;
        Some(())
    }
}
