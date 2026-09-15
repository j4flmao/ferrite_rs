use ferrite_framework::ConfigService;
use ferrite_macros::{inject, injectable};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AppInfo {
    #[schema(example = "micro-sqlx-products")]
    pub name: String,
    #[schema(example = "0.1.0")]
    pub version: String,
    #[schema(example = "ferrite-cqrs · ferrite-grpc · ferrite-kafka · ferrite-cache-redis · sqlx")]
    pub stack: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Health {
    #[schema(example = "ok")]
    pub status: String,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub timestamp: String,
    pub info: AppInfo,
}

#[injectable]
#[derive(Debug, Clone)]
pub struct AppService {
    config: ConfigService,
}

impl AppService {
    #[inject]
    pub fn new(config: ConfigService) -> Self {
        Self { config }
    }

    pub fn info(&self) -> AppInfo {
        AppInfo {
            name: self.config.get_or("APP_NAME", "micro-sqlx-products"),
            version: env!("CARGO_PKG_VERSION").into(),
            stack: "ferrite-cqrs · ferrite-grpc · ferrite-kafka · ferrite-cache-redis · sqlx"
                .into(),
        }
    }

    pub fn health(&self) -> Health {
        Health {
            status: "ok".into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            info: self.info(),
        }
    }

    pub fn gen_id(&self) -> String {
        Uuid::new_v4().to_string()
    }
}
