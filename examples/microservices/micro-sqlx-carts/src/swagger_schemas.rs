use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::app_service::{AppInfo, Health};
use crate::carts::dto::{
    AddToCartDto, Cart, CartItem, CartList, ConvertCartDto, CreateCartDto, UpdateCartItemDto,
};

#[derive(OpenApi)]
#[openapi(
    info(title = "micro-sqlx-carts · Ferrite CQRS/gRPC Microservice", version = "0.1.0", description = r#"
Ferrite microservice demo showcasing:

- **ferrite-cqrs**: CommandBus (CreateCart, AddToCart, UpdateCartItem, RemoveFromCart, ClearCart, ConvertCart), QueryBus (GetCart, GetUserCart, GetSessionCart, ListCarts), EventBus (CartCreated/CartUpdated → fan-out: Kafka publish + audit log)
- **ferrite-grpc**: GrpcServer, manual CartsGrpc service registered on port 50055
- **ferrite-kafka**: MemoryKafka `carts.events` topic — produced by CQRS event handlers and consumed by KafkaHandler
- **ferrite-cache-redis**: CacheService with TTL write-through (guest carts 24h TTL, logged-in carts 1h TTL; mutations invalidate the cart key)
- **ferrite-auth-jwt**: Bearer JWT auth on user-scoped cart reads, cart convert (guest → user), item mutations
- **ferrite-orm-sqlx**: Feature-gated drivers `sqlite` (default, no Docker) / `postgres` / `mysql` (docker compose)
- **ferrite-swagger**: Swagger UI at `GET /docs` → this OpenAPI document
- **Guest carts**: session_id based with 24h expiry; ConvertCart attaches the cart to a user_id
"#),
    paths(

    ),
    components(schemas(
        Health, AppInfo,
        Cart, CartItem, CreateCartDto, AddToCartDto, UpdateCartItemDto, ConvertCartDto, CartList,
    )),
    tags(
        (name = "Health", description = "Liveness & readiness probes"),
        (name = "Carts · CQRS", description = "Commands (create / add item / update / remove / clear / convert) + Queries (get / get by user / get by session / list) routed through ferrite-cqrs buses. Events fan-out to Kafka + audit append."),
        (name = "Cart Items", description = "Item-level mutations under /carts/:cart_id/items — add, update quantity, remove."),
        (name = "gRPC", description = "gRPC CartsService listens on 0.0.0.0:50055 — server reflection coming in v0.7 patch releases."),
        (name = "Kafka", description = "carts.events topic carries cart.created / cart.updated envelopes. Consumed by CartsEventsKafkaHandler (ring buffer)."),
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
                    .description(Some("Paste a JWT whose claims map to a Ferrite user_id (sub). Extra admin claims unlock admin endpoints."))
                    .build(),
            ),
        );
    }
}
