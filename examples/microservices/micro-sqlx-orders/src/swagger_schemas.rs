use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::app_service::{AppInfo, Health};
use crate::checkout::dto::{CheckoutDto, CheckoutSession};
use crate::orders::dto::{
    CancelOrderDto, CreateOrderDto, Order, OrderItem, OrderItemDto, OrderList, UpdateOrderStatusDto,
};

#[derive(OpenApi)]
#[openapi(
    info(title = "micro-sqlx-orders · Ferrite CQRS/gRPC Microservice", version = "0.1.0", description = r#"
Ferrite microservice demo showcasing:

- **ferrite-cqrs**: CommandBus (CreateOrder, UpdateOrderStatus, CancelOrder), QueryBus (GetOrder, ListOrders), EventBus (OrderCreated → fan-out: Kafka publish + Redis TTL reserve + audit log)
- **ferrite-grpc**: GrpcServer, manual OrdersGrpc service registered on port 50054
- **ferrite-kafka**: MemoryKafka `orders.events` topic — produced by CQRS event handler and consumed by KafkaHandler
- **ferrite-cache-redis**: CacheService with TTL write-through (GetOrder populates 5min cache; Insert/UpdateStatus/Cancel invalidates cache; pending orders get 15min reserve key)
- **ferrite-auth-jwt**: Bearer JWT auth (user_id from claims → own orders only; admin claims → user_id filter + status updates)
- **ferrite-orm-sqlx**: Feature-gated drivers `sqlite` (default, no Docker) / `postgres` / `mysql` (docker compose)
- **ferrite-swagger**: Swagger UI at `GET /docs` → this OpenAPI document
- **Checkout Flow**: POST /checkout/session → creates order via CQRS, publishes `checkout.completed` Kafka event, returns dummy Stripe-style payment URL
"#),
    paths(

    ),
    components(schemas(
        Health, AppInfo,
        Order, OrderItem, CreateOrderDto, OrderItemDto, UpdateOrderStatusDto, CancelOrderDto, OrderList,
        CheckoutDto, CheckoutSession,
    )),
    tags(
        (name = "Health", description = "Liveness & readiness probes"),
        (name = "Orders · CQRS", description = "Commands (create / status / cancel) + Queries (get / list) routed through ferrite-cqrs buses. AuthGuard enforces ownership; admin can list filtered by user_id and mutate status. Events fan-out to Kafka + audit append."),
        (name = "Checkout · Payment Flow", description = "Simulated checkout: POST /checkout/session validates cart, creates order via OrdersService CQRS, emits checkout.completed to Kafka, returns dummy payment URL (https://pay.example.com/order_xxx)."),
        (name = "gRPC", description = "gRPC OrdersService listens on 0.0.0.0:50054 — server reflection coming in v0.7 patch releases."),
        (name = "Kafka", description = "orders.events topic carries order.created, order.status_changed, checkout.completed envelopes. Consumed by OrdersEventsKafkaHandler (ring buffer)."),
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
                    .description(Some("Paste a JWT whose claims map to a Ferrite user_id (sub). Set extra.admin=true for admin endpoints."))
                    .build(),
            ),
        );
    }
}
