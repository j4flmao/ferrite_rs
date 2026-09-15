use ferrite_framework::Validate;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, Ordering};
use utoipa::ToSchema;

// ========== Domain entity ==========
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct User {
    #[schema(example = "1001")]
    pub id: i64,
    #[schema(example = "Alice Johnson")]
    pub name: String,
    #[schema(example = "alice@ferrite.dev")]
    pub email: String,
    #[schema(example = "https://example.com/avatars/alice.png", nullable = true)]
    pub avatar_url: Option<String>,
    #[schema(
        example = "Full-stack engineer, open source enthusiast.",
        nullable = true
    )]
    pub bio: Option<String>,
    #[schema(example = "user")]
    pub role: String,
    #[schema(example = "true")]
    pub is_active: bool,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub created_at: String,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub updated_at: String,
}

// ========== Internal (not exposed in DTOs) ==========
#[derive(Debug, Clone)]
pub struct UserInternal {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub password_hash: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub role: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl UserInternal {
    pub fn to_public(&self) -> User {
        User {
            id: self.id,
            name: self.name.clone(),
            email: self.email.clone(),
            avatar_url: self.avatar_url.clone(),
            bio: self.bio.clone(),
            role: self.role.clone(),
            is_active: self.is_active,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
}

// ========== HTTP DTOs ==========
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateUserDto {
    #[validate(not_empty)]
    #[validate(length(min = 2, max = 120))]
    #[schema(example = "Alice Johnson", min_length = 2, max_length = 120)]
    pub name: String,
    #[validate(email)]
    #[schema(example = "alice@ferrite.dev")]
    pub email: String,
    #[validate(length(min = 6, max = 128))]
    #[schema(example = "secret123", min_length = 6, max_length = 128)]
    pub password: String,
    #[schema(example = "user", nullable = true)]
    pub role: Option<String>,
    #[schema(example = "https://example.com/avatars/alice.png", nullable = true)]
    pub avatar_url: Option<String>,
    #[schema(
        example = "Full-stack engineer.",
        min_length = 0,
        max_length = 2000,
        nullable = true
    )]
    pub bio: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct UpdateUserDto {
    #[schema(
        example = "Alice Smith",
        min_length = 2,
        max_length = 120,
        nullable = true
    )]
    pub name: Option<String>,
    #[schema(example = "https://example.com/avatars/alice-v2.png", nullable = true)]
    pub avatar_url: Option<String>,
    #[schema(
        example = "Senior engineer now leading the team.",
        min_length = 0,
        max_length = 2000,
        nullable = true
    )]
    pub bio: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ChangePasswordDto {
    #[validate(length(min = 6))]
    #[schema(example = "oldsecret123", min_length = 6)]
    pub old_password: String,
    #[validate(length(min = 6, max = 128))]
    #[schema(example = "newsecret456", min_length = 6, max_length = 128)]
    pub new_password: String,
}

// ========== CQRS: Commands ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserCommand {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
}
impl ferrite_cqrs::Command for CreateUserCommand {}

static NEXT_USER_ID: AtomicI64 = AtomicI64::new(1000);

impl CreateUserCommand {
    pub fn next_id() -> i64 {
        NEXT_USER_ID.fetch_add(1, Ordering::SeqCst)
    }

    pub fn from_dto(dto: CreateUserDto, password_hash: String) -> Self {
        Self {
            id: Self::next_id(),
            name: dto.name,
            email: dto.email.to_lowercase(),
            password_hash,
            role: dto.role.unwrap_or_else(|| "user".into()),
            avatar_url: dto.avatar_url,
            bio: dto.bio,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserCommand {
    pub user_id: i64,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
}
impl ferrite_cqrs::Command for UpdateUserCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteUserCommand {
    pub user_id: i64,
}
impl ferrite_cqrs::Command for DeleteUserCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePasswordCommand {
    pub user_id: i64,
    pub old_password: String,
    pub new_password_hash: String,
}
impl ferrite_cqrs::Command for ChangePasswordCommand {}

// ========== CQRS: Queries ==========
#[derive(Debug, Clone)]
pub struct GetUserQuery {
    pub user_id: i64,
}
impl ferrite_cqrs::Query for GetUserQuery {
    type Result = Option<User>;
}

#[derive(Debug, Clone)]
pub struct ListUsersQuery {
    pub limit: usize,
    pub offset: usize,
}
impl ferrite_cqrs::Query for ListUsersQuery {
    type Result = UserList;
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserList {
    pub total: usize,
    #[schema(example = 20)]
    pub limit: usize,
    #[schema(example = 0)]
    pub offset: usize,
    pub items: Vec<User>,
}

// ========== CQRS: Events (fan-out) ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCreatedEvent {
    pub user: User,
    pub at: String,
}
impl ferrite_cqrs::Event for UserCreatedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUpdatedEvent {
    pub user_id: i64,
    pub user: User,
    pub changes: Vec<String>,
    pub at: String,
}
impl ferrite_cqrs::Event for UserUpdatedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDeletedEvent {
    pub user_id: i64,
    pub at: String,
}
impl ferrite_cqrs::Event for UserDeletedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordChangedEvent {
    pub user_id: i64,
    pub at: String,
}
impl ferrite_cqrs::Event for PasswordChangedEvent {}
