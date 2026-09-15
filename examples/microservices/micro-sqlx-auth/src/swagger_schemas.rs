use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::app_service::{AppInfo, Health};
use crate::auth::dto::{AuthResponse, LoginDto, RegisterDto, TokenValidationDto, User};
use crate::tokens::dto::{RefreshTokenDto, RevokeTokenDto, TokenStatusDto};

#[derive(OpenApi)]
#[openapi(
    info(title = "micro-sqlx-auth · Ferrite CQRS/gRPC Auth Microservice", version = "0.1.0", description = r#"
Ferrite auth microservice demo showcasing:

- **ferrite-cqrs**: CommandBus (RegisterUser, LoginUser), QueryBus (GetUser, ValidateToken), EventBus (UserCreated/UserLoggedIn → fan-out: Kafka publish + audit log)
- **ferrite-auth-jwt**: Argon2 password hashing + HS256 JWT sign/verify. Protected routes via AuthGuard/CurrentUser. Refresh tokens stored in Redis with TTL; revoke via blocklist key.
- **ferrite-grpc**: GrpcServer, manual AuthGrpc service registered on port 50052
- **ferrite-kafka**: MemoryKafka `auth.events` topic — produced by CQRS event handlers and consumed by KafkaHandler
- **ferrite-cache-redis**: CacheService for refresh token leases, revoked-token blocklist, and user write-through with TTL
- **ferrite-orm-sqlx**: Feature-gated drivers `sqlite` (default, no Docker) / `postgres` / `mysql` (docker compose)
- **ferrite-swagger**: Swagger UI at `GET /docs` → this OpenAPI document
- **/tokens**: refresh (rotate) and revoke (blocklist) a JWT/refresh token pair
"#),
    paths(

    ),
    components(schemas(
        Health, AppInfo,
        User, RegisterDto, LoginDto, AuthResponse, TokenValidationDto,
        RefreshTokenDto, RevokeTokenDto, TokenStatusDto,
    )),
    tags(
        (name = "Health", description = "Liveness & readiness probes"),
        (name = "Auth", description = "JWT register / login / me / validate — Argon2-hashed credentials, HS256 tokens."),
        (name = "Tokens", description = "Refresh-token rotation and revoke (blocklist via Redis)."),
        (name = "gRPC", description = "gRPC AuthService listens on 0.0.0.0:50052 — server reflection coming in v0.7 patch releases."),
        (name = "Kafka", description = "auth.events topic carries user.created / user.logged_in envelopes. Consumed by AuthEventsKafkaHandler (ring buffer)."),
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
