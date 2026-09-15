//! Ferrite Auth JWT — zero-setup HS256 JWT authentication for Ferrite apps.
//!
//! This crate ships the canonical Nest-style auth building blocks for Ferrite:
//!
//! * [`JwtClaims`] — standard claims shape (user id / email / expiry).
//! * [`JwtService`] — sign and verify HS256 JWTs using a secret read from
//!   env via [`ConfigService`]. Keys: `JWT_SECRET`, `JWT_EXPIRES_IN` (seconds).
//! * [`AuthGuard`] — a Ferrite [`GuardTrait`] that rejects requests without a
//!   valid `Authorization: Bearer <token>` header. Use it via
//!   `#[use_guards(AuthGuard)]` on controllers or individual routes.
//! * [`CurrentUser`] — Axum `FromRequestParts` extractor that resolves the
//!   claims straight from the incoming request handler parameters.
//! * [`AuthModule`] — registers `JwtService` and `AuthGuard` with DI so you
//!   can simply add it to your root module imports.
//!
//! # Usage
//!
//! ```ignore
//! use ferrite_framework::{module, Ferrite, bootstrap};
//! use ferrite_auth_jwt::{AuthModule, AuthGuard, CurrentUser};
//! use ferrite_macros::{use_guards, get, controller, impl_controller};
//!
//! #[controller("/users")]
//! pub struct UsersController;
//!
//! #[impl_controller]
//! impl UsersController {
//!     #[use_guards(AuthGuard)]
//!     #[get("/me")]
//!     pub async fn me(&self, CurrentUser(claims): CurrentUser) -> String {
//!         format!("hello {}", claims.email)
//!     }
//! }
//!
//! #[module(imports = [AuthModule], controllers = [UsersController])]
//! pub struct AppModule;
//! ```

use std::sync::Arc;

pub use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use ferrite_http::Guard as GuardTrait;
use ferrite_http::RequestCtx;
use ferrite_macros::{inject, injectable, module};
use fr_config::ConfigService;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors surfaced by [`JwtService::sign`] / [`JwtService::verify`].
#[derive(Debug, Error)]
pub enum JwtError {
    #[error("missing `JWT_SECRET` env variable")]
    MissingSecret,
    #[error("invalid token: {0}")]
    InvalidToken(String),
    #[error("missing bearer token")]
    MissingBearer,
    #[error("malformed authorization header")]
    MalformedHeader,
}

/// Standard claims encoded in every JWT issued by [`JwtService`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// Subject — typically the user id.
    pub sub: i64,
    /// Owner email (for display / audit logging).
    pub email: String,
    /// Unix timestamp (seconds) at which the token expires.
    pub exp: usize,
    /// Optional issuer tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
}

/// Signs and validates HS256 JWTs. Configure via env:
///
/// * `JWT_SECRET` — the HMAC secret (required for production).
/// * `JWT_EXPIRES_IN` — default lifetime in seconds, defaults to 86400.
#[injectable]
pub struct JwtService {
    config: ConfigService,
    _anchor: u8,
}

impl JwtService {
    #[inject]
    pub fn new(config: ConfigService) -> Self {
        Self {
            config,
            _anchor: Arc::new(0),
        }
    }

    /// HS256 secret used for signing + verification.
    pub fn secret(&self) -> String {
        self.config
            .get_or("JWT_SECRET", "ferrite-dev-secret-change-me")
    }

    /// Default lifetime (seconds) when signing a token.
    pub fn default_ttl_secs(&self) -> usize {
        self.config.get_or_parse("JWT_EXPIRES_IN", 86400usize)
    }

    /// HS256 header factory.
    pub fn header() -> Header {
        Header::default()
    }

    /// Build a JWT for `sub`/`email` with `exp = now + ttl`.
    pub fn sign(&self, sub: i64, email: impl Into<String>) -> Result<String, JwtError> {
        let ttl = self.default_ttl_secs();
        let exp = now_unix_secs().saturating_add(ttl);
        let claims = JwtClaims {
            sub,
            email: email.into(),
            exp,
            iss: self.config.get("JWT_ISSUER"),
        };
        let secret = self.secret();
        if secret.is_empty() {
            return Err(JwtError::MissingSecret);
        }
        let enc_key = EncodingKey::from_secret(secret.as_bytes());
        encode(&Self::header(), &claims, &enc_key)
            .map_err(|e| JwtError::InvalidToken(e.to_string()))
    }

    /// Verify a raw bearer token and return its claims.
    pub fn verify(&self, token: &str) -> Result<JwtClaims, JwtError> {
        let secret = self.secret();
        if secret.is_empty() {
            return Err(JwtError::MissingSecret);
        }
        let dec_key = DecodingKey::from_secret(secret.as_bytes());
        let mut validation = Validation::default();
        validation.validate_nbf = false;
        validation.validate_exp = true;
        let data = decode::<JwtClaims>(token, &dec_key, &validation)
            .map_err(|e| JwtError::InvalidToken(e.to_string()))?;
        Ok(data.claims)
    }
}

fn now_unix_secs() -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as usize)
        .unwrap_or(0)
}

/// Ferrite [`GuardTrait`] enforcing a valid JWT bearer token on the incoming
/// request. Attach with `#[use_guards(AuthGuard)]`.
#[injectable]
pub struct AuthGuard {
    jwt: JwtService,
    _anchor: u8,
}

impl AuthGuard {
    #[inject]
    pub fn new(jwt: JwtService) -> Self {
        Self {
            jwt,
            _anchor: Arc::new(0),
        }
    }

    /// Extract the raw bearer token (or a typed error) from an http request.
    pub fn extract_bearer(ctx: &RequestCtx) -> Result<String, JwtError> {
        let header = ctx
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(JwtError::MissingBearer)?;
        let trimmed = header.trim();
        trimmed
            .strip_prefix("Bearer ")
            .map(|s| s.trim().to_string())
            .ok_or(JwtError::MalformedHeader)
    }
}

#[async_trait]
impl GuardTrait for AuthGuard {
    async fn can_activate(&self, ctx: &RequestCtx) -> bool {
        match Self::extract_bearer(ctx) {
            Ok(token) => self.jwt.verify(&token).is_ok(),
            Err(_) => false,
        }
    }
}

/// Axum extractor that resolves the current user's claims from the
/// `Authorization: Bearer <token>` header. Useful inside handlers protected
/// by [`AuthGuard`]. Resolves its own [`JwtService`] from the environment so
/// it works out of the box without task-local bridges.
#[derive(Debug, Clone)]
pub struct CurrentUser(pub JwtClaims);

impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "missing bearer token".into()))?;
        let token = header
            .trim()
            .strip_prefix("Bearer ")
            .map(|s| s.trim())
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "malformed authorization header".into(),
            ))?;

        let config = ConfigService::load();
        let jwt = JwtService::new(Arc::new(config));
        let claims = jwt
            .verify(token)
            .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
        Ok(CurrentUser(claims))
    }
}

/// Module registering the default JWT providers with Ferrite DI.
#[module(
    controllers = [],
    providers = [JwtService, AuthGuard],
)]
pub struct AuthModule;

#[cfg(test)]
mod tests {
    use super::*;

    fn with_test_env<R>(f: impl FnOnce(&std::path::Path) -> R) -> R {
        let dir =
            std::env::temp_dir().join(format!("ferrite-auth-jwt-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".env"),
            "JWT_SECRET=test-secret\nJWT_EXPIRES_IN=3600\n",
        )
        .unwrap();
        let result = f(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn roundtrip_sign_verify() {
        with_test_env(|dir| {
            let cfg = ConfigService::load_from(dir);
            let svc = JwtService::new(Arc::new(cfg));
            let token = svc.sign(42, "alice@example.com").unwrap();
            let claims = svc.verify(&token).unwrap();
            assert_eq!(claims.sub, 42);
            assert_eq!(claims.email, "alice@example.com");
        });
    }

    #[test]
    fn rejects_tampered_token() {
        with_test_env(|dir| {
            let cfg = ConfigService::load_from(dir);
            let svc = JwtService::new(Arc::new(cfg));
            let token = svc.sign(42, "alice@example.com").unwrap();
            let mut bytes = token.into_bytes();
            if let Some(last) = bytes.last_mut() {
                *last ^= 0x1;
            }
            let bad = String::from_utf8(bytes).unwrap();
            assert!(matches!(svc.verify(&bad), Err(JwtError::InvalidToken(_))));
        });
    }
}
