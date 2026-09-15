use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::app_service::{AppInfo, Health};
use crate::auth::dto::{AuthResponse, LoginDto, RegisterDto, User};
use crate::products::dto::{CreateProductDto, Product, ProductList, UpdateStockDto};

#[derive(OpenApi)]
#[openapi(
    info(title = "micro-sqlx-products · Ferrite CQRS/gRPC Microservice", version = "0.1.0", description = r#"
Ferrite microservice demo showcasing:

- **ferrite-cqrs**: CommandBus (CreateProduct, UpdateProductStock), QueryBus (GetProduct, ListProducts), EventBus (ProductCreated → fan-out: Kafka publish + audit log)
- **ferrite-grpc**: GrpcServer, manual ProductsGrpc service registered on port 50051
- **ferrite-kafka**: MemoryKafka `products.events` topic — produced by CQRS event handler and consumed by KafkaHandler
- **ferrite-cache-redis**: CacheService with TTL write-through (GetProduct populates 5min cache; Insert/UpdateStock invalidates cache)
- **ferrite-auth-jwt**: Bearer JWT auth (register/login → token → 401 on protected routes)
- **ferrite-orm-sqlx**: Feature-gated drivers `sqlite` (default, no Docker) / `postgres` / `mysql` (docker compose)
- **ferrite-swagger**: Swagger UI at `GET /docs` → this OpenAPI document
"#),
    paths(

    ),
    components(schemas(
        Health, AppInfo, User, RegisterDto, LoginDto, AuthResponse,
        Product, CreateProductDto, UpdateStockDto, ProductList,
    )),
    tags(
        (name = "Health", description = "Liveness & readiness probes"),
        (name = "Auth", description = "JWT register / login / me — Bearer tokens"),
        (name = "Products · CQRS", description = "Commands (mutations) + Queries (reads) routed through ferrite-cqrs buses. On success Command dispatches an Event → fan-out (Kafka publish + audit append)."),
        (name = "gRPC", description = "gRPC ProductsService listens on 0.0.0.0:50051 — server reflection coming in v0.7 patch releases."),
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
                    .description(Some(
                        "Paste your JWT from POST /auth/login or /auth/register (token field)",
                    ))
                    .build(),
            ),
        );
    }
}
