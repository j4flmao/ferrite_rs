use crate::auth::dto::AuthResponse;
use crate::auth::service::AuthService;
use crate::tokens::dto::{RefreshTokenDto, RevokeTokenDto, TokenStatusDto};
use axum::http::HeaderMap;
use ferrite_auth_jwt::AuthGuard;
use ferrite_framework::{controller, impl_controller, inject, HttpError, Json};

#[controller("/tokens")]
pub struct TokensController {
    auth: AuthService,
}

#[impl_controller]
impl TokensController {
    #[inject]
    pub fn new(auth: AuthService) -> Self {
        Self { auth }
    }

    #[post("/refresh")]
    pub async fn refresh(
        &self,
        Json(body): Json<RefreshTokenDto>,
    ) -> Result<Json<AuthResponse>, HttpError> {
        Ok(Json(self.auth.refresh_token(&body.refresh_token).await?))
    }

    #[post("/revoke")]
    #[use_guards(AuthGuard)]
    pub async fn revoke(
        &self,
        headers: HeaderMap,
        Json(body): Json<RevokeTokenDto>,
    ) -> Result<Json<TokenStatusDto>, HttpError> {
        let token = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer ").map(|s| s.trim().to_string()))
            .ok_or_else(|| HttpError::unauthorized("missing bearer token"))?;
        let ttl = body.ttl_seconds.unwrap_or(86400);
        self.auth.revoke_token(&token, Some(ttl)).await?;
        Ok(Json(TokenStatusDto {
            status: "revoked".into(),
            at: chrono::Utc::now().to_rfc3339(),
            ttl_seconds: ttl,
        }))
    }
}
