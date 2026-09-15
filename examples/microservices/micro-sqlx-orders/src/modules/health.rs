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
        Html(r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Ferrite · micro-sqlx-orders</title></head><body style="background:#fbf5eb;color:#3f2a1b;font-family:ui-sans-serif,system-ui,-apple-system,Arial;padding:48px"><h1 style="margin:0 0 8px">🛒  micro-sqlx-orders microservice</h1><p style="margin:4px 0 16px;color:#7f5730">ferrite-cqrs · ferrite-grpc · ferrite-kafka · ferrite-cache-redis · sqlx · auth-jwt</p><ul style="line-height:1.8"><li><code>GET /healthz</code> / <code>GET /health</code> — health probes</li><li><code>POST /orders/commands/create</code> (CQRS CreateOrder → Kafka orders.events)</li><li><code>POST /orders/commands/:id/status</code> (admin, CQRS UpdateOrderStatus)</li><li><code>POST /orders/commands/:id/cancel</code> (own order, CQRS CancelOrder)</li><li><code>GET /orders/queries/:id</code> (CQRS GetOrder)</li><li><code>GET /orders/queries</code> (CQRS ListOrders, paginated)</li><li><code>POST /checkout/session</code> — create checkout session with dummy payment URL</li><li><code>GET /docs</code> · <code>GET /openapi.json</code> — Swagger UI</li><li>gRPC OrdersService on <code>0.0.0.0:50054</code></li></ul></body></html>"#.into())
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
