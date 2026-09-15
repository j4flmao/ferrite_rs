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
        Html(r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Ferrite · micro-sqlx-users</title></head><body style="background:#fbf5eb;color:#3f2a1b;font-family:ui-sans-serif,system-ui,-apple-system,Arial;padding:48px"><h1 style="margin:0 0 8px">👤 micro-sqlx-users microservice</h1><p style="margin:4px 0 16px;color:#7f5730">ferrite-cqrs · ferrite-grpc · ferrite-kafka · ferrite-cache-redis · sqlx · ferrite-auth-jwt</p><ul style="line-height:1.8"><li><code>GET /healthz</code> / <code>GET /health</code> — health probes</li><li><code>POST /users/commands/create</code> (AuthGuard admin, CQRS CreateUser → Kafka users.events)</li><li><code>PUT /users/commands/:id/update</code> (AuthGuard, own profile or admin, CQRS UpdateUser → Kafka users.events)</li><li><code>DELETE /users/commands/:id</code> (AuthGuard admin, CQRS DeleteUser)</li><li><code>GET /users/queries/:id</code> (public, CQRS GetUser)</li><li><code>GET /users/queries</code> (AuthGuard admin, CQRS ListUsers paginated)</li><li><code>POST /users/commands/:id/password</code> (AuthGuard, change own password)</li><li><code>GET /profiles/:user_id</code> (public profile)</li><li><code>GET /docs</code> · <code>GET /openapi.json</code> — Swagger UI</li><li>gRPC UsersService on <code>0.0.0.0:50053</code></li></ul></body></html>"#.into())
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
