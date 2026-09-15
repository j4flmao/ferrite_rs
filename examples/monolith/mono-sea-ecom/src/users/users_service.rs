use std::sync::atomic::{AtomicI64, Ordering};

use chrono::Utc;
use dashmap::DashMap;
use ferrite_framework::{inject, injectable};

use super::models::User;

static NEXT_ID: AtomicI64 = AtomicI64::new(1);

#[injectable]
pub struct UsersService {
    store: DashMap<i64, User>,
}

impl Default for UsersService {
    fn default() -> Self {
        Self::new()
    }
}

impl UsersService {
    #[inject]
    pub fn new() -> Self {
        let store = DashMap::new();
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        store.insert(
            id,
            User {
                id,
                email: "admin@example.com".into(),
                password_hash: "$argon2id$v=19$m=65536,t=2,p=1$c29tZXNhbHQ$R0lOS1Q".into(),
                name: "Admin".into(),
                role: "admin".into(),
                created_at: Utc::now(),
            },
        );
        Self {
            store: store.into(),
        }
    }

    pub async fn find_all(&self) -> Vec<User> {
        self.store.iter().map(|r| r.value().clone()).collect()
    }

    pub async fn find_one(&self, id: i64) -> Option<User> {
        self.store.get(&id).map(|r| r.value().clone())
    }

    pub async fn find_by_email(&self, email: &str) -> Option<User> {
        self.store
            .iter()
            .find(|r| r.value().email.eq_ignore_ascii_case(email))
            .map(|r| r.value().clone())
    }

    pub async fn create(
        &self,
        email: String,
        name: String,
        password_hash: String,
        role: String,
    ) -> User {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let user = User {
            id,
            email,
            password_hash,
            name,
            role,
            created_at: Utc::now(),
        };
        self.store.insert(id, user.clone());
        user
    }

    pub async fn update(
        &self,
        id: i64,
        name: Option<String>,
        role: Option<String>,
    ) -> Option<User> {
        let mut user = self.store.get_mut(&id)?;
        if let Some(n) = name {
            user.name = n;
        }
        if let Some(r) = role {
            user.role = r;
        }
        Some(user.value().clone())
    }

    pub async fn delete(&self, id: i64) -> bool {
        self.store.remove(&id).is_some()
    }
}
