use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::app_service::{AppInfo, Health};
use crate::profiles::dto::{ProfileDto, ProfileListDto};
use crate::users::dto::{ChangePasswordDto, CreateUserDto, UpdateUserDto, User, UserList};

#[derive(OpenApi)]
#[openapi(
    info(title = "micro-sqlx-users · Ferrite CQRS/gRPC Users Microservice", version = "0.1.0", description = r#"
Ferrite users microservice demo showcasing:

- **ferrite-cqrs**: CommandBus (CreateUser, UpdateUser, DeleteUser, ChangePassword), QueryBus (GetUser, ListUsers), EventBus (UserCreated/UserUpdated/UserDeleted/PasswordChanged → fan-out: Kafka publish + audit log)
- **ferrite-grpc**: GrpcServer, manual UsersGrpc service registered on port 50053
- **ferrite-kafka**: MemoryKafka `users.events` topic — produced by CQRS event handlers and consumed by KafkaHandler
- **ferrite-cache-redis**: CacheService with TTL write-through (GetUser populates 5min cache; inserts/updates invalidate)
- **ferrite-auth-jwt**: Bearer JWT auth on mutations and user-scoped reads
- **ferrite-orm-sqlx**: Feature-gated drivers `sqlite` (default, no Docker) / `postgres` / `mysql` (docker compose)
- **ferrite-swagger**: Swagger UI at `GET /docs` → this OpenAPI document
- **/profiles**: public-facing profile projection built on top of the user aggregate
"#),
    paths(

    ),
    components(schemas(
        Health, AppInfo,
        User, CreateUserDto, UpdateUserDto, ChangePasswordDto, UserList,
        ProfileDto, ProfileListDto,
    )),
    tags(
        (name = "Health", description = "Liveness & readiness probes"),
        (name = "Users · CQRS", description = "Commands (create / update / delete / change password) + Queries (get / list) routed through ferrite-cqrs buses. Events fan-out to Kafka + audit append."),
        (name = "Profiles", description = "Public profile projection of users — no password_hash, no emails."),
        (name = "gRPC", description = "gRPC UsersService listens on 0.0.0.0:50053 — server reflection coming in v0.7 patch releases."),
        (name = "Kafka", description = "users.events topic carries user.created / user.updated envelopes. Consumed by UsersEventsKafkaHandler (ring buffer)."),
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
                    .description(Some("Paste a JWT whose claims map to a Ferrite user_id (sub). Admin claims unlock list/delete endpoints."))
                    .build(),
            ),
        );
    }
}
