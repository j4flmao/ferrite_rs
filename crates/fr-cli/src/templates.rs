//! File-generating templates for `fr new`.

/// Catalog of all templates the CLI can scaffold (for `fr templates --list`/`--gallery`).
/// Returns (slug, human_title, short_description, files_count).
pub fn gallery() -> Vec<(&'static str, &'static str, &'static str, usize)> {
    vec![
        (
            "api",
            "REST API (default)",
            "HTTP API with users CRUD, health endpoint, DTOs, validation and e2e tests.",
            17,
        ),
        (
            "microservice",
            "Microservices playground",
            "gRPC transport + NATS message patterns with sample Greeter + order event handlers.",
            9,
        ),
        (
            "workspace",
            "Monorepo workspace",
            "Cargo workspace skeleton with apps/api and libs/common; ideal for scaling teams.",
            11,
        ),
        (
            "graphql",
            "GraphQL (planned)",
            "Placeholder GraphQL template — currently falls back to the REST API skeleton.",
            17,
        ),
    ]
}

/// Return a Vec of (relative path, contents) for a new project.
pub fn files(template: &str, name: &str) -> Vec<(String, String)> {
    match template {
        "microservice" => microservice_files(name),
        "workspace" => workspace_files(name),
        "graphql" => {
            eprintln!(
                "note: template `graphql` is not built yet — using the default `api` template"
            );
            api_files(name)
        }
        _ => api_files(name),
    }
}

fn api_files(name: &str) -> Vec<(String, String)> {
    vec![
        ("Cargo.toml".into(), cargo_toml(name)),
        ("ferrite.toml".into(), ferrite_toml(name)),
        (".env.example".into(), env_example()),
        (".gitignore".into(), gitignore()),
        (".cargo/config.toml".into(), cargo_config()),
        ("src/main.rs".into(), main_rs()),
        ("src/app_module.rs".into(), app_module()),
        ("src/app_service.rs".into(), app_service()),
        ("src/modules/mod.rs".into(), modules_mod()),
        ("src/modules/health.rs".into(), modules_health()),
        ("src/users/mod.rs".into(), users_mod()),
        ("src/users/models.rs".into(), users_models()),
        ("src/users/users_controller.rs".into(), users_controller()),
        ("src/users/users_service.rs".into(), users_service()),
        ("src/users/dto.rs".into(), users_dto()),
        ("tests/health_e2e.rs".into(), health_e2e()),
    ]
}

fn cargo_toml(name: &str) -> String {
    format!(
        r##"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
ferrite-framework = "=0.1.0"
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
tokio = {{ version = "1", features = ["full"] }}
tower-http = {{ version = "0.6", features = ["cors", "trace"] }}
"##
    )
}

fn ferrite_toml(name: &str) -> String {
    format!(
        r##"[app]
name = "{name}"

[server]
transport = "http"
host = "0.0.0.0"
port = 3000
"##
    )
}

fn env_example() -> String {
    r#"
APP_ENV=development
PORT=3000

DATABASE_URL=postgres://user:pass@localhost:5432/app
JWT_SECRET=change-me
JWT_EXPIRES_IN=3600
"#
    .to_string()
}

fn gitignore() -> String {
    "/target\n.env\nCargo.lock\n".to_string()
}

fn main_rs() -> String {
    r#"#[ferrite_framework::bootstrap]
async fn main() {
    let app = ferrite_framework::Ferrite::create::<AppModule>().await;
    app.listen("0.0.0.0:3000").await.expect("failed to bind port 3000");
}

mod app_module;
mod app_service;
mod modules;
mod users;

use app_module::AppModule;
"#
    .to_string()
}

fn app_module() -> String {
    r#"use ferrite_framework::module;

use crate::app_service::AppService;
use crate::modules::HealthController;
use crate::users::{UsersController, UsersService};

#[module(
    controllers = [HealthController, UsersController],
    providers = [AppService, UsersService],
)]
pub struct AppModule;
"#
    .to_string()
}

fn app_service() -> String {
    r#"use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Health {
    pub status: &'static str,
    pub env: String,
}

#[ferrite_framework::injectable]
pub struct AppService {
    config: ferrite_framework::ConfigService,
}

impl AppService {
    #[ferrite_framework::inject]
    pub fn new(config: ferrite_framework::ConfigService) -> Self {
        Self { config }
    }

    pub fn health(&self) -> Health {
        Health {
            status: "ok",
            env: self.config.get_or("APP_ENV", "development"),
        }
    }
}
"#
    .to_string()
}

fn modules_mod() -> String {
    r#"pub mod health;

pub use health::HealthController;
"#
    .to_string()
}

fn modules_health() -> String {
    r#"use ferrite_framework::{controller, impl_controller, inject, Json};

use crate::app_service::{AppService, Health};

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
    pub async fn healthz(&self) -> Json<Health> {
        Json(self.service.health())
    }
}
"#
    .to_string()
}

fn users_mod() -> String {
    r#"pub mod dto;
pub mod models;
mod users_controller;
mod users_service;

pub use users_controller::UsersController;
pub use users_service::UsersService;
"#
    .to_string()
}

fn users_models() -> String {
    r#"use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct User {
    pub id: u64,
    pub email: String,
    pub name: String,
}
"#
    .to_string()
}

fn users_service() -> String {
    r#"use ferrite_framework::{inject, injectable};

use super::models::User;

#[injectable]
pub struct UsersService {
    config: ferrite_framework::ConfigService,
}

impl UsersService {
    #[inject]
    pub fn new(config: ferrite_framework::ConfigService) -> Self {
        Self { config }
    }

    #[allow(dead_code)]
    pub fn app_env(&self) -> String {
        self.config.get_or("APP_ENV", "development")
    }

    pub async fn find_all(&self) -> Vec<User> {
        vec![
            User { id: 1, email: "ana@example.com".into(), name: "Ana".into() },
            User { id: 2, email: "bob@example.com".into(), name: "Bob".into() },
        ]
    }

    pub async fn find_one(&self, id: u64) -> Option<User> {
        self.find_all().await.into_iter().find(|u| u.id == id)
    }

    pub async fn create(&self, email: String, name: String) -> User {
        User { id: 99, email, name }
    }
}
"#
    .to_string()
}

fn users_controller() -> String {
    r#"use ferrite_framework::{controller, impl_controller, inject, HttpError, Json, Path};

use super::dto::{CreateUserDto, UpdateUserDto};
use super::models::User;
use super::UsersService;

#[controller("/users")]
pub struct UsersController {
    service: UsersService,
}

#[impl_controller]
impl UsersController {
    #[inject]
    pub fn new(service: UsersService) -> Self {
        Self { service }
    }

    #[get("/")]
    pub async fn find_all(&self) -> Json<Vec<User>> {
        Json(self.service.find_all().await)
    }

    #[get("/{id}")]
    pub async fn find_one(&self, Path(id): Path<u64>) -> Result<Json<User>, HttpError> {
        match self.service.find_one(id).await {
            Some(u) => Ok(Json(u)),
            None => Err(HttpError::not_found("user")),
        }
    }

    #[post("/")]
    pub async fn create(&self, Json(dto): Json<CreateUserDto>) -> Json<User> {
        Json(self.service.create(dto.email, dto.name).await)
    }

    #[patch("/{id}")]
    pub async fn update(
        &self,
        Path(id): Path<u64>,
        Json(dto): Json<UpdateUserDto>,
    ) -> Result<Json<User>, HttpError> {
        match self.service.find_one(id).await {
            Some(mut u) => {
                if let Some(n) = dto.name {
                    u.name = n;
                }
                Ok(Json(u))
            }
            None => Err(HttpError::not_found("user")),
        }
    }
}
"#
    .to_string()
}

fn users_dto() -> String {
    r#"use ferrite_framework::Validate;
use serde::Deserialize;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateUserDto {
    #[validate(email)]
    pub email: String,
    #[validate(not_empty)]
    pub name: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserDto {
    pub name: Option<String>,
}
"#
    .to_string()
}

fn health_e2e() -> String {
    r#"// E2E tests land with `ferrite-testing` (v0.4). Until then this is a placeholder.
#[test]
fn placeholder_e2e() {
    assert!(true);
}
"#
    .to_string()
}

// ---------------------------------------------------------------------------
// Microservice template: gRPC + NATS message-pattern services.
// ---------------------------------------------------------------------------

fn microservice_files(name: &str) -> Vec<(String, String)> {
    vec![
        ("Cargo.toml".into(), microservice_cargo_toml(name)),
        ("ferrite.toml".into(), microservice_ferrite_toml(name)),
        (".env.example".into(), microservice_env_example()),
        (".gitignore".into(), gitignore()),
        (".cargo/config.toml".into(), cargo_config()),
        ("src/main.rs".into(), microservice_main_rs()),
        ("src/app_module.rs".into(), microservice_app_module()),
        ("src/greeter.rs".into(), microservice_greeter()),
        ("src/order_events.rs".into(), microservice_order_events()),
    ]
}

fn microservice_cargo_toml(name: &str) -> String {
    format!(
        r##"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
ferrite-framework = "=0.1.0"
ferrite-grpc = "=0.1.0"
ferrite-nats = "=0.1.0"
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
tokio = {{ version = "1", features = ["full"] }}
tonic = {{ version = "0.11", default-features = false, features = ["prost", "transport"] }}
prost = "0.13"
async-trait = "0.1"
"##
    )
}

fn microservice_ferrite_toml(name: &str) -> String {
    format!(
        r##"[app]
name = "{name}"

[server]
transport = "grpc"
host = "0.0.0.0"
port = 50051

[microservices]
nats_poll_ms = 50
grpc_health_timeout_ms = 30000
"##
    )
}

fn microservice_env_example() -> String {
    r#"
APP_ENV=development
GRPC_PORT=50051
NATS_POLL_MS=50
GRPC_HEALTH_TIMEOUT_MS=30000
"#
    .to_string()
}

fn microservice_main_rs() -> String {
    r#"#[ferrite_framework::bootstrap]
async fn main() {
    let app = ferrite_framework::Ferrite::create::<AppModule>().await;

    let grpc = app.get::<ferrite_grpc::GrpcServer>();
    let nats = app.get::<ferrite_nats::NatsServer>();

    let grpc_handle = tokio::spawn(async move {
        if let Err(e) = grpc.start("0.0.0.0:50051").await {
            eprintln!("[grpc] server stopped with error: {e:?}");
        }
    });

    let nats_handle = tokio::spawn(async move { nats.run().await });

    let http = app.listen("0.0.0.0:3000");
    tokio::select! {
        r = http => r.expect("http server failed"),
        _ = grpc_handle => {},
        _ = nats_handle => {},
    }
}

mod app_module;
mod greeter;
mod order_events;

use app_module::AppModule;
"#
    .to_string()
}

fn microservice_app_module() -> String {
    r#"use ferrite_framework::module;
use ferrite_grpc::GrpcModule;
use ferrite_nats::NatsModule;

use crate::greeter::GreeterService;
use crate::order_events::OrdersHandler;

#[module(
    imports = [GrpcModule, NatsModule],
    providers = [GreeterService, OrdersHandler],
)]
pub struct AppModule;
"#
    .to_string()
}

fn microservice_greeter() -> String {
    r#"use ferrite_framework::injectable;
use ferrite_grpc::{submit_grpc_service, DummyNamedService};

#[injectable]
pub struct GreeterService {
    config: ferrite_framework::ConfigService,
}

submit_grpc_service!(DummyNamedService);

impl GreeterService {
    #[ferrite_framework::inject]
    pub fn new(config: ferrite_framework::ConfigService) -> Self {
        Self { config }
    }

    pub fn hello(&self, name: &str) -> String {
        format!("Hello, {name}! — from {}", self.config.get_or("APP_ENV", "development"))
    }
}
"#
    .to_string()
}

fn microservice_order_events() -> String {
    r#"use async_trait::async_trait;
use ferrite_framework::injectable;
use ferrite_nats::{submit_nats_handler, MessageHandler, NatsError, NatsMessage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCreated {
    pub id: u64,
    pub user_id: u64,
    pub total_cents: u64,
}

#[injectable]
pub struct OrdersHandler {
    config: ferrite_framework::ConfigService,
}

#[async_trait]
impl MessageHandler<OrderCreated> for OrdersHandler {
    const PATTERN: &'static str = "orders.created";

    async fn handle(&self, msg: NatsMessage<OrderCreated>) -> Result<(), NatsError> {
        tracing_or_print(msg.payload.id, msg.payload.user_id, msg.payload.total_cents);
        let _ = &self.config;
        Ok(())
    }
}

fn tracing_or_print(id: u64, user: u64, total: u64) {
    println!("[OrdersHandler] order {id} by user {user} for {total} cents");
}

submit_nats_handler!(OrdersHandler, OrderCreated);
"#
    .to_string()
}

// ---------------------------------------------------------------------------
// Shared: hot-reload Cargo config (incremental + cranelift + platform linker)
// ---------------------------------------------------------------------------

fn cargo_config() -> String {
    r#"# Ferrite: hot-reload optimizations for `fr up`
# Keep release builds on the default LLVM backend; only tweak dev profile here.

[build]
# Incremental is on by default, but pin it to be explicit for newcomers.
incremental = true

# NOTE: Uncomment the rustflags below ONLY if you have the cranelift codegen
# backend installed. It is *not* shipped with stable Rust by default.
#   rustup component add rustc-codegen-cranelift-preview --toolchain nightly
# or:  cargo install rustc_codegen_cranelift
#
# [target.x86_64-unknown-linux-gnu]
# rustflags = ["-C", "linker=clang", "-C", "link-arg=-fuse-ld=lld", "-C", "codegen-backend=cranelift"]
#
# [target.x86_64-apple-darwin]
# rustflags = ["-C", "link-arg=-fuse-ld=lld", "-C", "codegen-backend=cranelift"]
#
# [target.x86_64-pc-windows-msvc]
# rustflags = ["-C", "linker=rust-lld.exe", "-C", "codegen-backend=cranelift"]
#
# [target.x86_64-pc-windows-gnu]
# rustflags = ["-C", "linker=lld", "-C", "link-arg=-fuse-ld=lld", "-C", "codegen-backend=cranelift"]
"#
    .to_string()
}

// ---------------------------------------------------------------------------
// Workspace template: monorepo skeleton with apps/ + libs/.
// ---------------------------------------------------------------------------

fn workspace_files(workspace_name: &str) -> Vec<(String, String)> {
    vec![
        ("Cargo.toml".into(), workspace_root_cargo_toml()),
        ("README.md".into(), workspace_readme(workspace_name)),
        (".gitignore".into(), gitignore()),
        (".env.example".into(), env_example()),
        (".cargo/config.toml".into(), cargo_config()),
        (
            "apps/api/Cargo.toml".into(),
            workspace_app_cargo_toml(workspace_name),
        ),
        ("apps/api/src/main.rs".into(), workspace_app_main_rs()),
        (
            "apps/api/src/app_module.rs".into(),
            workspace_app_app_module(),
        ),
        (
            "apps/api/src/app_service.rs".into(),
            workspace_app_service(),
        ),
        ("libs/common/Cargo.toml".into(), workspace_lib_cargo_toml()),
        (
            "libs/common/src/lib.rs".into(),
            workspace_lib_rs(workspace_name),
        ),
    ]
}

fn workspace_root_cargo_toml() -> String {
    r#"[workspace]
resolver = "2"
members = [
    "apps/*",
    "libs/*",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["Your Name <you@example.com>"]

[workspace.dependencies]
ferrite-framework = "=0.1.0"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
common = { path = "libs/common" }
"#
    .to_string()
}

fn workspace_readme(name: &str) -> String {
    format!(
        r##"# {name}

Ferrite monorepo workspace.

## Structure

```
apps/
  api/      # HTTP API application (run it with `cargo run -p api`)
libs/
  common/   # Shared domain types, DTOs and utilities.
```

## Quick start

```sh
cargo run -p api
```
"##
    )
}

fn workspace_app_cargo_toml(workspace: &str) -> String {
    let _ = workspace;
    r#"[package]
name = "api"
version.workspace = true
edition.workspace = true
authors.workspace = true

[dependencies]
ferrite-framework = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
common = { workspace = true }
tower-http = { version = "0.6", features = ["cors", "trace"] }
"#
    .to_string()
}

fn workspace_app_main_rs() -> String {
    r#"#[ferrite_framework::bootstrap]
async fn main() {
    let app = ferrite_framework::Ferrite::create::<AppModule>().await;
    app.listen("0.0.0.0:3000").await.expect("failed to bind port 3000");
}

mod app_module;
mod app_service;

use app_module::AppModule;
"#
    .to_string()
}

fn workspace_app_app_module() -> String {
    r#"use ferrite_framework::module;

use crate::app_service::{AppController, AppService};

#[module(
    controllers = [AppController],
    providers = [AppService],
)]
pub struct AppModule;
"#
    .to_string()
}

fn workspace_app_service() -> String {
    r#"use common::HelloResponse;
use ferrite_framework::{controller, impl_controller, inject, injectable, Json};

#[injectable]
pub struct AppService {
    config: ferrite_framework::ConfigService,
}

impl AppService {
    #[inject]
    pub fn new(config: ferrite_framework::ConfigService) -> Self {
        Self { config }
    }

    pub fn hello(&self) -> HelloResponse {
        HelloResponse {
            message: format!(
                "hello from {} workspace",
                self.config.get_or("APP_ENV", "development")
            ),
        }
    }
}

#[controller("/")]
pub struct AppController {
    service: AppService,
}

#[impl_controller]
impl AppController {
    #[inject]
    pub fn new(service: AppService) -> Self {
        Self { service }
    }

    #[get("/")]
    pub async fn root(&self) -> Json<HelloResponse> {
        Json(self.service.hello())
    }
}
"#
    .to_string()
}

fn workspace_lib_cargo_toml() -> String {
    r#"[package]
name = "common"
version.workspace = true
edition.workspace = true
authors.workspace = true

[dependencies]
serde = { workspace = true }
"#
    .to_string()
}

fn workspace_lib_rs(name: &str) -> String {
    let _ = name;
    r#"use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct HelloResponse {
    pub message: String,
}
"#
    .to_string()
}
