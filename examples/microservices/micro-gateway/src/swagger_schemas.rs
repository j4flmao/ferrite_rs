use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::app_service::{AppInfo, Health};
use crate::gateway::dto::{GatewayServicesResponse, ServiceStatus};

#[derive(OpenApi)]
#[openapi(
    info(title = "micro-gateway · Ferrite API Gateway Microservice", version = "0.1.0", description = r#"
Ferrite API Gateway microservice showcasing:

- **HTTP Reverse Proxy**: Routes traffic based on path prefix to backend microservices
  - `/auth/**` → AUTH_SERVICE_URL
  - `/users/**` → USER_SERVICE_URL
  - `/products/**` → PRODUCT_SERVICE_URL
  - `/orders/**` → ORDER_SERVICE_URL
  - `/carts/**` → CART_SERVICE_URL
- **ferrite-auth-jwt**: Bearer JWT auth on protected routes (users, product mutations, orders, carts)
- **ferrite-cache-redis**: CacheService with TTL write-through
- **ferrite-throttler**: Rate limiting on all gateway endpoints
- **ferrite-swagger**: Swagger UI at `GET /docs` → this OpenAPI document
- **Service Registry**: `GET /services` returns live health status of all registered backend services
"#),
    paths(

    ),
    components(schemas(
        Health, AppInfo, ServiceStatus, GatewayServicesResponse,
    )),
    tags(
        (name = "Health", description = "Liveness & readiness probes"),
        (name = "Gateway · Proxy", description = "Service registry status and gateway configuration"),
        (name = "Auth (proxy)", description = "Proxied to AUTH_SERVICE_URL — JWT register / login / me endpoints"),
        (name = "Users (proxy)", description = "Proxied to USER_SERVICE_URL — protected by Bearer JWT"),
        (name = "Products (proxy)", description = "Proxied to PRODUCT_SERVICE_URL — GET is public; mutations require Bearer JWT"),
        (name = "Orders (proxy)", description = "Proxied to ORDER_SERVICE_URL — protected by Bearer JWT"),
        (name = "Carts (proxy)", description = "Proxied to CART_SERVICE_URL — protected by Bearer JWT"),
        (name = "Swagger", description = "Self-hosted Swagger UI CDN endpoint"),
    ),
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .description(Some("Paste your JWT from the proxied POST /auth/login or /auth/register endpoint (token field)"))
                    .build(),
            ),
        );
    }
}
