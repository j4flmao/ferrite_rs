use crate::auth::dto::{AuthResponse, LoginDto, RegisterDto, TokenValidationDto, User, UsersRepo};
use anyhow::{anyhow, Result as AnyResult};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use ferrite_auth_jwt::{JwtError, JwtService};
use ferrite_cache_redis::CacheService;
use ferrite_framework::{inject, injectable, HttpError};
use rand::Rng;
use std::time::Duration;

#[injectable]
pub struct AuthService {
    users: UsersRepo,
    jwt: JwtService,
    cache: CacheService,
}

impl AuthService {
    #[inject]
    pub fn new(users: UsersRepo, jwt: JwtService, cache: CacheService) -> Self {
        Self { users, jwt, cache }
    }

    pub fn hash_password(&self, password: &str) -> AnyResult<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!("hash error: {e}"))?;
        Ok(hash.to_string())
    }

    pub fn verify_password(&self, password: &str, hash: &str) -> bool {
        match PasswordHash::new(hash) {
            Ok(parsed) => Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok(),
            Err(_) => false,
        }
    }

    fn generate_refresh_token(&self) -> String {
        use std::fmt::Write as FmtWrite;
        let mut rng = rand::thread_rng();
        let bytes: [u8; 32] = rng.gen();
        let mut s = String::with_capacity(64);
        for b in bytes {
            let _ = write!(s, "{:02x}", b);
        }
        s
    }

    fn refresh_token_key(&self, refresh_token: &str) -> String {
        format!("refresh:{refresh_token}")
    }

    fn revoked_key(&self, token: &str) -> String {
        format!("revoked:{token}")
    }

    fn sanitize_user(user: User) -> User {
        User {
            password_hash: String::new(),
            ..user
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
        let refresh_token = self.generate_refresh_token();
        let rt_key = self.refresh_token_key(&refresh_token);
        let ttl: u64 = std::env::var("JWT_REFRESH_EXPIRES_IN_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(604800);
        let _ = self.cache.set_with_ttl(&rt_key, &user.id, ttl).await;
        Ok(AuthResponse {
            token,
            refresh_token,
            user: Self::sanitize_user(user),
        })
    }

    pub async fn login(
        &self,
        dto: LoginDto,
        ip: Option<String>,
    ) -> Result<AuthResponse, HttpError> {
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
        let refresh_token = self.generate_refresh_token();
        let rt_key = self.refresh_token_key(&refresh_token);
        let ttl: u64 = std::env::var("JWT_REFRESH_EXPIRES_IN_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(604800);
        let _ = self.cache.set_with_ttl(&rt_key, &user.id, ttl).await;
        let _ip = ip;
        Ok(AuthResponse {
            token,
            refresh_token,
            user: Self::sanitize_user(user),
        })
    }

    pub async fn me(&self, user_id: i64) -> Option<User> {
        self.users
            .find_by_id(user_id)
            .await
            .map(Self::sanitize_user)
    }

    pub async fn validate_token(&self, dto: TokenValidationDto) -> Result<Option<User>, HttpError> {
        let revoked = self
            .cache
            .get::<String>(&self.revoked_key(&dto.token))
            .await;
        if matches!(revoked, Ok(Some(_))) {
            return Err(HttpError::unauthorized("token revoked"));
        }
        match self.jwt.verify(&dto.token) {
            Ok(claims) => {
                let user = self.users.find_by_id(claims.sub).await;
                Ok(user.map(Self::sanitize_user))
            }
            Err(JwtError::InvalidToken(_)) => Ok(None),
            Err(e) => Err(jwt_to_http(e)),
        }
    }

    pub async fn refresh_token(&self, refresh_token: &str) -> Result<AuthResponse, HttpError> {
        let rt_key = self.refresh_token_key(refresh_token);
        let user_id: i64 = self
            .cache
            .get::<i64>(&rt_key)
            .await
            .map_err(|_| HttpError::internal("cache error"))?
            .ok_or_else(|| HttpError::unauthorized("invalid or expired refresh token"))?;
        let _ = self.cache.delete(&rt_key).await;
        let user = self
            .users
            .find_by_id(user_id)
            .await
            .ok_or_else(|| HttpError::unauthorized("user not found"))?;
        let token = self
            .jwt
            .sign(user.id, user.email.as_str())
            .map_err(jwt_to_http)?;
        let new_refresh = self.generate_refresh_token();
        let new_rt_key = self.refresh_token_key(&new_refresh);
        let ttl: u64 = std::env::var("JWT_REFRESH_EXPIRES_IN_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(604800);
        let _ = self.cache.set_with_ttl(&new_rt_key, &user.id, ttl).await;
        Ok(AuthResponse {
            token,
            refresh_token: new_refresh,
            user: Self::sanitize_user(user),
        })
    }

    pub async fn revoke_token(
        &self,
        token: &str,
        ttl_seconds: Option<u64>,
    ) -> Result<(), HttpError> {
        let key = self.revoked_key(token);
        let ttl = ttl_seconds.unwrap_or(86400);
        let marker = "1".to_string();
        let _ = self.cache.set_with_ttl(&key, &marker, ttl).await;
        let _ = tokio::time::sleep(Duration::from_micros(100)).await;
        Ok(())
    }
}

fn jwt_to_http(e: JwtError) -> HttpError {
    match e {
        JwtError::MissingSecret => HttpError::internal("JWT_SECRET not configured"),
        JwtError::InvalidToken(_) => HttpError::unauthorized("invalid credentials"),
        other => HttpError::internal(format!("jwt: {other}")),
    }
}
