use ferrite_framework::Validate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserDto {
    pub name: Option<String>,
    pub role: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub role: String,
    pub created_at: i64,
}

impl From<crate::users::models::User> for UserResponse {
    fn from(u: crate::users::models::User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            name: u.name,
            role: u.role,
            created_at: u.created_at.timestamp(),
        }
    }
}
