use crate::auth::dto::{AuthResponse, LoginDto, RegisterDto, User, UsersRepo};
use anyhow::{anyhow, Result as AnyResult};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use ferrite_auth_jwt::{JwtError, JwtService};
use ferrite_framework::{inject, injectable, HttpError};

#[injectable]
pub struct AuthService {
    users: UsersRepo,
    jwt: JwtService,
}

impl AuthService {
    #[inject]
    pub fn new(users: UsersRepo, jwt: JwtService) -> Self {
        Self { users, jwt }
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

    pub async fn register(&self, dto: RegisterDto) -> Result<AuthResponse, HttpError> {
        if self.users.email_exists(&dto.email) {
            return Err(HttpError::bad_request("email already registered"));
        }
        let hash = self
            .hash_password(&dto.password)
            .map_err(|_| HttpError::internal("password hashing failed"))?;
        let user = self.users.create(&dto, &hash);
        let token = self
            .jwt
            .sign(user.id, user.email.as_str())
            .map_err(jwt_to_http)?;
        Ok(AuthResponse {
            token,
            user: User {
                password_hash: String::new(),
                ..user
            },
        })
    }

    pub async fn login(&self, dto: LoginDto) -> Result<AuthResponse, HttpError> {
        let user = self
            .users
            .find_by_email(&dto.email)
            .ok_or_else(|| HttpError::unauthorized("invalid email or password"))?;
        if !self.verify_password(&dto.password, &user.password_hash) {
            return Err(HttpError::unauthorized("invalid email or password"));
        }
        let token = self
            .jwt
            .sign(user.id, user.email.as_str())
            .map_err(jwt_to_http)?;
        Ok(AuthResponse {
            token,
            user: User {
                password_hash: String::new(),
                ..user
            },
        })
    }

    pub fn me(&self, user_id: i64) -> Option<User> {
        self.users.find_by_id(user_id).map(|u| User {
            password_hash: String::new(),
            ..u
        })
    }
}

fn jwt_to_http(e: JwtError) -> HttpError {
    match e {
        JwtError::MissingSecret => HttpError::internal("JWT_SECRET not configured"),
        JwtError::InvalidToken(_) => HttpError::unauthorized("invalid credentials"),
        other => HttpError::internal(format!("jwt: {other}")),
    }
}
