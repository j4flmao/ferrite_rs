use ferrite_framework::Validate;
use ferrite_macros::{inject, injectable};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, Ordering};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct User {
    #[schema(example = "1001")]
    pub id: i64,
    #[schema(example = "John Doe")]
    pub name: String,
    #[schema(example = "john@ferrite.dev")]
    pub email: String,
    pub password_hash: String,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub created_at: String,
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
    pub user: User,
}

#[injectable]
#[derive(Debug)]
pub struct UsersRepo {
    inner: dashmap::DashMap<i64, User>,
    by_email: dashmap::DashMap<String, i64>,
    next_id: AtomicI64,
}

impl UsersRepo {
    #[inject]
    pub fn new() -> Self {
        Self {
            inner: dashmap::DashMap::new().into(),
            by_email: dashmap::DashMap::new().into(),
            next_id: AtomicI64::new(1000).into(),
        }
    }

    pub fn create(&self, dto: &RegisterDto, password_hash: &str) -> User {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let user = User {
            id,
            name: dto.name.clone(),
            email: dto.email.to_lowercase(),
            password_hash: password_hash.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.by_email.insert(user.email.clone(), id);
        self.inner.insert(id, user.clone());
        user
    }

    pub fn find_by_email(&self, email: &str) -> Option<User> {
        let key = email.to_lowercase();
        let id = self.by_email.get(&key)?;
        self.inner.get(id.value()).map(|x| x.clone())
    }

    pub fn find_by_id(&self, id: i64) -> Option<User> {
        self.inner.get(&id).map(|x| x.clone())
    }

    pub fn email_exists(&self, email: &str) -> bool {
        self.by_email.contains_key(&email.to_lowercase())
    }
}
