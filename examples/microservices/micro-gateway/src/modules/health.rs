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
        let auth_url =
            std::env::var("AUTH_SERVICE_URL").unwrap_or_else(|_| "http://localhost:3005".into());
        let user_url =
            std::env::var("USER_SERVICE_URL").unwrap_or_else(|_| "http://localhost:3006".into());
        let product_url =
            std::env::var("PRODUCT_SERVICE_URL").unwrap_or_else(|_| "http://localhost:3004".into());
        let order_url =
            std::env::var("ORDER_SERVICE_URL").unwrap_or_else(|_| "http://localhost:3007".into());
        let cart_url =
            std::env::var("CART_SERVICE_URL").unwrap_or_else(|_| "http://localhost:3008".into());

        Html(format!(
            r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Ferrite · micro-gateway</title></head><body style="background:#f0f4f8;color:#1a202c;font-family:ui-sans-serif,system-ui,-apple-system,Arial;padding:48px"><h1 style="margin:0 0 8px">🌐  micro-gateway API Gateway</h1><p style="margin:4px 0 16px;color:#4a5568">ferrite-gateway · ferrite-auth-jwt · ferrite-cache-redis · ferrite-swagger</p><h2 style="margin:24px 0 8px">Proxied Routes</h2><ul style="line-height:1.8"><li><code>GET /healthz</code> / <code>GET /health</code> — health probes</li><li><code>GET /services</code> — registered backend services status</li><li><code>/auth/**</code> → Auth service at <code>{auth_url}</code></li><li><code>/users/**</code> → User service at <code>{user_url}</code></li><li><code>/products/**</code> → Product service at <code>{product_url}</code></li><li><code>/orders/**</code> → Order service at <code>{order_url}</code></li><li><code>/carts/**</code> → Cart service at <code>{cart_url}</code></li><li><code>GET /docs</code> · <code>GET /openapi.json</code> — Swagger UI</li></ul></body></html>"#
        ))
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
