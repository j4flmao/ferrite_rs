use axum::response::Html;
use ferrite_framework::{controller, impl_controller, inject, Json};

use crate::app_service::{AppService, Health};

#[controller("/")]
pub struct RootController {
    service: AppService,
}

#[impl_controller]
impl RootController {
    #[inject]
    pub fn new(service: AppService) -> Self {
        Self { service }
    }

    #[get("/")]
    pub async fn index(&self) -> Html<String> {
        Html(r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Ferrite · micro-sqlx-auth</title></head><body style="background:#eaf4fb;color:#1b3a4b;font-family:ui-sans-serif,system-ui,-apple-system,Arial;padding:48px"><h1 style="margin:0 0 8px">🔐  micro-sqlx-auth microservice</h1><p style="margin:4px 0 16px;color:#3c6e8f">ferrite-cqrs · ferrite-grpc · ferrite-kafka · ferrite-cache-redis · sqlx · auth-jwt</p><ul style="line-height:1.8"><li><code>GET /healthz</code> / <code>GET /health</code> — health probes</li><li><code>POST /auth/register</code> · <code>POST /auth/login</code> · <code>GET /auth/me</code> (AuthGuard)</li><li><code>POST /auth/validate</code> — internal token validation endpoint</li><li><code>POST /tokens/refresh</code> — refresh token grant</li><li><code>POST /tokens/revoke</code> (AuthGuard) — revoke current token (Redis blacklist)</li><li>gRPC AuthService on <code>0.0.0.0:50052</code> (ValidateToken, GetUser, Register, Login)</li><li>Kafka <code>auth.events</code> topic (UserCreated, UserLoggedIn)</li><li><code>GET /docs</code> · <code>GET /openapi.json</code> — Swagger UI</li></ul></body></html>"#.into())
    }

    #[get("/healthz")]
    pub async fn healthz(&self) -> Json<Health> {
        Json(self.service.health())
    }
}

#[controller("/health")]
pub struct HealthController {
    service: AppService,
}

#[impl_controller]
impl HealthController {
    #[inject]
    pub fn new(service: AppService) -> Self {
        Self { service }
    }

    #[get("/")]
    pub async fn health(&self) -> Json<Health> {
        Json(self.service.health())
    }
}
