use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ServiceStatus {
    #[schema(example = "Auth Service")]
    pub name: String,
    #[schema(example = "http://localhost:3005")]
    pub url: String,
    #[schema(example = "ok")]
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GatewayServicesResponse {
    pub services: Vec<ServiceStatus>,
    #[schema(example = "micro-gateway")]
    pub gateway: String,
    #[schema(example = "2026-09-14T15:30:00Z")]
    pub timestamp: String,
}

impl GatewayServicesResponse {
    pub fn example() -> Self {
        Self {
            gateway: "micro-gateway".into(),
            timestamp: "2026-09-14T15:30:00Z".into(),
            services: vec![
                ServiceStatus {
                    name: "Auth Service".into(),
                    url: "http://localhost:3005".into(),
                    status: "ok".into(),
                },
                ServiceStatus {
                    name: "User Service".into(),
                    url: "http://localhost:3006".into(),
                    status: "ok".into(),
                },
                ServiceStatus {
                    name: "Product Service".into(),
                    url: "http://localhost:3004".into(),
                    status: "ok".into(),
                },
                ServiceStatus {
                    name: "Order Service".into(),
                    url: "http://localhost:3007".into(),
                    status: "ok".into(),
                },
                ServiceStatus {
                    name: "Cart Service".into(),
                    url: "http://localhost:3008".into(),
                    status: "ok".into(),
                },
            ],
        }
    }
}
