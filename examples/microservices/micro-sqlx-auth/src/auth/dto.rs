use ferrite_cache_redis::CacheService;
use ferrite_framework::Validate;
use ferrite_macros::{inject, injectable};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, Ordering};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct User {
    #[schema(example = "1001")]
    pub id: i64,
    #[schema(example = "john doe")]
    pub name: String,
    #[schema(example = "john@ferrite.dev")]
    pub email: String,
    pub password_hash: String,
    #[schema(example = "user")]
    pub role: String,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub created_at: String,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct RegisterDto {
    #[validate(not_empty)]
    #[validate(length(min = 2, max = 120))]
    #[schema(example = "john doe", min_length = 2, max_length = 120)]
    pub name: String,
    #[validate(email)]
    #[schema(example = "john@ferrite.dev")]
    pub email: String,
    #[validate(length(min = 6, max = 128))]
    #[schema(example = "secret123", min_length = 6, max_length = 128)]
    pub password: String,
    #[validate(length(max = 32))]
    #[schema(example = "user", min_length = 0, max_length = 32)]
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "user".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct LoginDto {
    #[validate(email)]
    #[schema(example = "john@ferrite.dev")]
    pub email: String,
    #[validate(length(min = 6))]
    #[schema(example = "secret123", min_length = 6)]
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthResponse {
    #[schema(example = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...")]
    pub token: String,
    #[schema(example = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...refresh...")]
    pub refresh_token: String,
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct TokenValidationDto {
    #[validate(not_empty)]
    #[schema(example = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...")]
    pub token: String,
}

#[injectable]
pub struct UsersRepo {
    inner: dashmap::DashMap<i64, User>,
    by_email: dashmap::DashMap<String, i64>,
    next_id: AtomicI64,
    cache: CacheService,
}

impl UsersRepo {
    #[inject]
    pub fn new(cache: CacheService) -> Self {
        Self {
            inner: dashmap::DashMap::new().into(),
            by_email: dashmap::DashMap::new().into(),
            next_id: AtomicI64::new(1000).into(),
            cache,
        }
    }

    fn cache_key(id: i64) -> String {
        format!("user:{id}")
    }

    pub fn create(&self, dto: &RegisterDto, password_hash: &str) -> User {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let now = chrono::Utc::now().to_rfc3339();
        let role = if dto.role.trim().is_empty() {
            "user".into()
        } else {
            dto.role.clone()
        };
        let user = User {
            id,
            name: dto.name.clone(),
            email: dto.email.to_lowercase(),
            password_hash: password_hash.to_string(),
            role,
            created_at: now.clone(),
            updated_at: now,
        };
        self.by_email.insert(user.email.clone(), id);
        self.inner.insert(id, user.clone());
        let cache = self.cache.clone();
        let cache_user = user.clone();
        tokio::spawn(async move {
            let _ = cache
                .set_with_ttl(&Self::cache_key(cache_user.id), &cache_user, 300)
                .await;
        });
        user
    }

    pub fn find_by_email(&self, email: &str) -> Option<User> {
        let key = email.to_lowercase();
        let id = self.by_email.get(&key)?;
        self.inner.get(id.value()).map(|x| x.clone())
    }

    pub async fn find_by_id(&self, id: i64) -> Option<User> {
        let key = Self::cache_key(id);
        if let Ok(Some(cached)) = self.cache.get::<User>(&key).await {
            return Some(cached);
        }
        let from_mem = self.inner.get(&id).map(|x| x.clone());
        if let Some(u) = &from_mem {
            let _ = self.cache.set_with_ttl(&key, u, 300).await;
        }
        from_mem
    }

    pub fn find_by_id_sync(&self, id: i64) -> Option<User> {
        self.inner.get(&id).map(|x| x.clone())
    }

    pub fn email_exists(&self, email: &str) -> bool {
        self.by_email.contains_key(&email.to_lowercase())
    }
}
