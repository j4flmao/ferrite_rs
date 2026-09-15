use crate::users::dto::{
    ChangePasswordCommand, ChangePasswordDto, CreateUserCommand, CreateUserDto, DeleteUserCommand,
    GetUserQuery, ListUsersQuery, UpdateUserCommand, UpdateUserDto, User, UserList,
};
use crate::users::repo::UsersRepo;
use anyhow::{anyhow, Result as AnyResult};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use ferrite_cqrs::{CommandBus, QueryBus};
use ferrite_framework::{inject, injectable, HttpError};

#[injectable]
pub struct UsersService {
    repo: UsersRepo,
    commands: CommandBus,
    queries: QueryBus,
}

impl UsersService {
    #[inject]
    pub fn new(repo: UsersRepo, commands: CommandBus, queries: QueryBus) -> Self {
        Self {
            repo,
            commands,
            queries,
        }
    }

    fn hash_password(&self, password: &str) -> AnyResult<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!("hash error: {e}"))?;
        Ok(hash.to_string())
    }

    fn verify_password(&self, password: &str, hash: &str) -> bool {
        match PasswordHash::new(hash) {
            Ok(parsed) => Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok(),
            Err(_) => false,
        }
    }

    pub async fn create_user(&self, dto: CreateUserDto) -> Result<User, HttpError> {
        if self.repo.email_exists(&dto.email) {
            return Err(HttpError::bad_request("email already registered"));
        }
        let hash = self
            .hash_password(&dto.password)
            .map_err(|_| HttpError::internal("password hashing failed"))?;
        let cmd = CreateUserCommand::from_dto(dto, hash);
        let user_id = cmd.id;
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        match self.queries.dispatch(GetUserQuery { user_id }).await {
            Ok(Some(u)) => Ok(u),
            Ok(None) => Err(HttpError::not_found(format!(
                "user {user_id} not found after insert"
            ))),
            Err(e) => Err(HttpError::internal(format!("query failed: {e}"))),
        }
    }

    pub async fn update_user(&self, user_id: i64, dto: UpdateUserDto) -> Result<User, HttpError> {
        if self.repo.get(user_id).await.is_none() {
            return Err(HttpError::not_found(format!("user {user_id} not found")));
        }
        let cmd = UpdateUserCommand {
            user_id,
            name: dto.name,
            avatar_url: dto.avatar_url,
            bio: dto.bio,
        };
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        match self.queries.dispatch(GetUserQuery { user_id }).await {
            Ok(Some(u)) => Ok(u),
            Ok(None) => Err(HttpError::not_found(format!("user {user_id} not found"))),
            Err(e) => Err(HttpError::internal(format!("query failed: {e}"))),
        }
    }

    pub async fn delete_user(&self, user_id: i64) -> Result<(), HttpError> {
        if self.repo.get(user_id).await.is_none() {
            return Err(HttpError::not_found(format!("user {user_id} not found")));
        }
        let cmd = DeleteUserCommand { user_id };
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        Ok(())
    }

    pub async fn get_user(&self, user_id: i64) -> Option<User> {
        self.queries
            .dispatch(GetUserQuery { user_id })
            .await
            .unwrap_or(None)
    }

    pub async fn list_users(&self, limit: usize, offset: usize) -> UserList {
        self.queries
            .dispatch(ListUsersQuery { limit, offset })
            .await
            .unwrap_or(UserList {
                total: 0,
                limit,
                offset,
                items: vec![],
            })
    }

    pub async fn change_password(
        &self,
        user_id: i64,
        dto: ChangePasswordDto,
    ) -> Result<(), HttpError> {
        let internal = self
            .repo
            .get_internal(user_id)
            .await
            .ok_or_else(|| HttpError::not_found(format!("user {user_id} not found")))?;
        if !self.verify_password(&dto.old_password, &internal.password_hash) {
            return Err(HttpError::bad_request("old password is incorrect"));
        }
        let new_hash = self
            .hash_password(&dto.new_password)
            .map_err(|_| HttpError::internal("password hashing failed"))?;
        let cmd = ChangePasswordCommand {
            user_id,
            old_password: dto.old_password,
            new_password_hash: new_hash,
        };
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        Ok(())
    }
}
