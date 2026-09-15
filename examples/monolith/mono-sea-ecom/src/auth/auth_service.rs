use std::sync::Mutex;

use anyhow::{anyhow, Result};
use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Algorithm, Argon2, Params, PasswordHasher, Version};
use ferrite_auth_jwt::{JwtError, JwtService};
use ferrite_framework::{inject, injectable, HttpError};

use crate::users::UsersService;

use super::dto::{AuthResponse, LoginDto, RegisterDto};

#[injectable]
pub struct AuthService {
    users: UsersService,
    jwt: JwtService,
    argon: Mutex<Argon2<'static>>,
}

impl AuthService {
    #[inject]
    pub fn new(users: UsersService, jwt: JwtService) -> Self {
        let params = Params::new(65536, 2, 1, Some(32)).expect("argon2 params");
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        Self {
            users,
            jwt,
            argon: Mutex::new(argon).into(),
        }
    }

    fn hash_password(&self, pw: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon = self.argon.lock().unwrap();
        let hash = argon
            .hash_password(pw.as_bytes(), &salt)
            .map_err(|e| anyhow!("argon2 hash: {e}"))?;
        Ok(hash.to_string())
    }

    fn verify_password(&self, pw: &str, hash: &str) -> bool {
        use argon2::PasswordVerifier;
        let argon = self.argon.lock().unwrap();
        let parsed = match argon2::PasswordHash::new(hash) {
            Ok(p) => p,
            Err(_) => return false,
        };
        argon.verify_password(pw.as_bytes(), &parsed).is_ok()
    }

    pub async fn register(&self, dto: RegisterDto) -> Result<AuthResponse, HttpError> {
        if self.users.find_by_email(&dto.email).await.is_some() {
            return Err(HttpError::bad_request("email already registered"));
        }
        let hashed = self
            .hash_password(&dto.password)
            .map_err(|_| HttpError::internal("password hashing failed"))?;
        let user = self
            .users
            .create(dto.email.clone(), dto.name, hashed, "user".into())
            .await;
        let token = self.jwt.sign(user.id, &user.email).map_err(jwt_to_http)?;
        Ok(AuthResponse {
            token,
            token_type: "Bearer",
            expires_in: self.jwt.default_ttl_secs(),
            user_id: user.id,
            email: user.email,
        })
    }

    pub async fn login(&self, dto: LoginDto) -> Result<AuthResponse, HttpError> {
        let user = self
            .users
            .find_by_email(&dto.email)
            .await
            .ok_or_else(|| HttpError::unauthorized("invalid email or password"))?;
        if !self.verify_password(&dto.password, &user.password_hash) {
            return Err(HttpError::unauthorized("invalid email or password"));
        }
        let token = self.jwt.sign(user.id, &user.email).map_err(jwt_to_http)?;
        Ok(AuthResponse {
            token,
            token_type: "Bearer",
            expires_in: self.jwt.default_ttl_secs(),
            user_id: user.id,
            email: user.email,
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
