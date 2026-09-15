# Ferrite

[![CI](https://github.com/j4flmao/ferrite_rs/actions/workflows/ci.yml/badge.svg)](https://github.com/j4flmao/ferrite_rs/actions/workflows/ci.yml)
[![Security](https://github.com/j4flmao/ferrite_rs/actions/workflows/security.yml/badge.svg)](https://github.com/j4flmao/ferrite_rs/actions/workflows/security.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)

> A batteries-included backend framework for Rust: modules, compile-time
> dependency injection, and macro-based routing on top of Axum, Tokio, and Tower.

Ferrite gives Rust backends a structured, modular shape. You describe your
application with **modules**, **providers**, **controllers**, and
**cross-cutting concerns** (guards, interceptors, pipes), and Ferrite wires the
whole dependency graph together at **compile time**. If a dependency is missing
or a cycle exists, you get a build error instead of a runtime panic.

Everything else — configuration, validation, OpenAPI, authentication,
persistence, caching, job queues, scheduling, WebSockets, gRPC, messaging,
health checks, and testing — ships as an official `ferrite-*` crate you can pull
in with a single CLI command.

## Highlights

- **Compile-time dependency injection** — providers are registered and resolved
  at build time; no runtime reflection.
- **Modules & providers** — declare `imports`, `providers`, `controllers`, and
  `exports` on a struct with the `#[module]` macro.
- **Macro-based routing** — `#[controller]`, `#[get]`, `#[post]`, `#[patch]`,
  `#[delete]`, `#[impl_controller]`.
- **Cross-cutting concerns** — guards, interceptors, middleware, pipes, and
  exception filters, composed with attribute macros.
- **Configuration** — layered `.env` and `ferrite.toml` via `ConfigService`.
- **Validation & OpenAPI** — derive validation on DTOs and export an OpenAPI 3.1
  spec from route metadata.
- **Authentication** — JWT guards and OAuth2/OIDC flows.
- **Persistence** — one repository abstraction with SQLx, Diesel, and SeaORM
  adapters.
- **Realtime & messaging** — WebSocket gateways, gRPC, NATS, and Kafka
  transports share the same module/controller model.
- **Operational tooling** — health indicators, rate limiting, Redis cache,
  background jobs, and cron scheduling.
- **CLI-first workflow** — scaffold projects, generate artifacts, manage the
  ecosystem, run a hot-reloading dev server, and drive migrations with one `fr`
  binary.
- **Single static binary** — ship a self-contained executable with no runtime
  dependencies.

## Quick start

### 1. Install the CLI

```bash
cargo install fr-cli
fr --version
```

### 2. Scaffold a project

```bash
fr new my-api
cd my-api
```

`fr new` supports the `api` (default), `microservice`, and `workspace`
templates, and an optional database layer with `--pm sqlx | sea-orm | diesel`.

### 3. Run it

```bash
fr up
# GET http://localhost:3000/health  ->  {"status":"ok","env":"development"}
```

## A minimal application

```rust
use ferrite_framework::{
    bootstrap, controller, impl_controller, inject, injectable, module, Ferrite, Json,
};

#[injectable]
pub struct AppService {
    config: ferrite_framework::ConfigService,
}

impl AppService {
    #[inject]
    pub fn new(config: ferrite_framework::ConfigService) -> Self {
        Self { config }
    }

    pub fn health(&self) -> String {
        self.config.get_or("APP_ENV", "development")
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
    pub async fn health(&self) -> Json<String> {
        Json(self.service.health())
    }
}

#[module(controllers = [HealthController], providers = [AppService])]
pub struct AppModule;

#[bootstrap]
async fn main() {
    let app = Ferrite::create::<AppModule>().await;
    app.listen("0.0.0.0:3000")
        .await
        .expect("failed to bind 0.0.0.0:3000");
}
```

## CLI

| Command | Description |
| --- | --- |
| `fr new <name>` | Scaffold a new project (`--template`, `--pm`, `--no-git`) |
| `fr up` | Run the dev server with hot reload |
| `fr build` | Build the project (`--release`, `--target`) |
| `fr run` | Run the project in the foreground |
| `fr test [suite]` | Run tests (`--watch`, `--coverage`) |
| `fr forge <schematic> <name>` | Generate artifacts (`module`, `controller`, `service`, `resource`, `guard`, ...) |
| `fr add <package>` / `fr remove <package>` | Add or remove an ecosystem crate |
| `fr db <action>` | Database lifecycle (`migrate`, `migrate:create`, `revert`, `seed`, `studio`) |
| `fr openapi export` | Export an OpenAPI 3.1 spec |
| `fr doctor` | Check toolchain, database, and environment |
| `fr info` | Print project and crate version report |
| `fr lint` / `fr fmt` | Clippy presets / `cargo fmt` |
| `fr generate config` | Scaffold `ferrite.toml` and `.env.example` |
| `fr templates` | List available project templates |

## Ecosystem crates

Install any of these with `fr add <short-name>`. Versions are pinned to the
matching framework release so the ecosystem stays consistent.

| Short name | Crate | Purpose |
| --- | --- | --- |
| — | `ferrite-framework` | The batteries-included entry point (re-exports core crates) |
| — | `fr-core` | Kernel: bootstrap, DI container, module graph, lifecycle hooks |
| — | `ferrite-macros` | `#[module]`, `#[controller]`, `#[injectable]`, ... |
| — | `ferrite-http` | Axum-based HTTP transport |
| — | `fr-config` | `.env` + `ferrite.toml` layered configuration |
| — | `ferrite-orm` | ORM abstraction (`Entity`, `Repository`, `Store`) |
| `auth-jwt` | `ferrite-auth-jwt` | JWT strategy, `AuthGuard`, current-user extractor |
| `auth-oauth` | `ferrite-auth-oauth` | OAuth2 / OIDC login flows |
| `orm-sqlx` | `ferrite-orm-sqlx` | SQLx adapter |
| `orm-diesel` | `ferrite-orm-diesel` | Diesel adapter |
| `orm-sea` | `ferrite-orm-sea` | SeaORM adapter |
| `swagger` | `ferrite-swagger` | OpenAPI 3.1 generation and Swagger UI route |
| `validation` | `ferrite-validation` | DTO validation and validation pipe |
| `cache-redis` | `ferrite-cache-redis` | Cache service and cache interceptor |
| `queue` | `ferrite-queue` | Background job queues and processors |
| `cron` | `ferrite-cron` | Scheduled tasks |
| `ws` | `ferrite-ws` | WebSocket gateways |
| `grpc` | `ferrite-grpc` | tonic-based gRPC transport |
| `nats` | `ferrite-nats` | NATS message transport |
| `kafka` | `ferrite-kafka` | Kafka message transport |
| `i18n` | `ferrite-i18n` | Localization and translation service |
| `throttler` | `ferrite-throttler` | Rate limiting guard |
| `health` | `ferrite-health` | `/health` endpoint and health indicators |
| `cqrs` | `ferrite-cqrs` | Command / query / event bus |
| `testing` | `ferrite-testing` | `TestingModule` builder and HTTP test client |

## Project layout

`fr new my-api` produces a single-crate REST service:

```text
my-api/
├── Cargo.toml
├── ferrite.toml
├── .env.example
├── src/
│   ├── main.rs                 # #[bootstrap] + Ferrite::create::<AppModule>()
│   ├── app_module.rs           # #[module] root
│   ├── app_service.rs
│   ├── modules/
│   │   └── health.rs           # GET /health
│   └── users/
│       ├── mod.rs
│       ├── models.rs
│       ├── dto.rs
│       ├── users_controller.rs
│       └── users_service.rs
└── tests/
    └── health_e2e.rs
```

The `workspace` template instead emits a multi-crate monorepo with `apps/` and
`libs/`.

## Examples

The repository ships runnable reference applications:

- `examples/monolith/mono-sqlx-ecom` — e-commerce monolith on SQLx
- `examples/monolith/mono-diesel-ecom` — e-commerce monolith on Diesel
- `examples/monolith/mono-sea-ecom` — e-commerce monolith on SeaORM
- `examples/monolith/mono-ws-chat` — WebSocket chat
- `examples/microservices/micro-gateway` — API gateway
- `examples/microservices/micro-sqlx-{auth,products,orders,carts,users}` — a
  service fleet exercising gRPC, JWT auth, caching, and CQRS

```bash
cargo run -p micro-gateway
```

## Documentation

- API reference: `cargo doc --workspace --no-deps --open`.
- Long-form guides are in the works and will land in this repository soon.

## Development status

Ferrite is at **v0.1.0**. The `ferrite-*` crates and the `fr` CLI are published
to crates.io; ecosystem crates are released in lockstep and pinned to `=0.1.0`,
so a project's dependencies always resolve to one consistent release line.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for build
instructions, the checks CI runs, and pull request guidelines.

## Security

Please report vulnerabilities privately. See [SECURITY.md](SECURITY.md) for the
policy, scope, and response expectations.

## License

Licensed under the [MIT License](LICENSE).
