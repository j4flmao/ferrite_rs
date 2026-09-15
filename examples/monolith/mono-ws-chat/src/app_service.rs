use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HealthInfo {
    pub status: &'static str,
    pub env: String,
    pub app: String,
    pub version: &'static str,
}

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub name: String,
    pub description: &'static str,
    pub features: &'static [&'static str],
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

    pub fn health(&self) -> HealthInfo {
        HealthInfo {
            status: "ok",
            env: self.config.get_or("APP_ENV", "development"),
            app: self.config.get_or("APP_NAME", "mono-ws-chat"),
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    pub fn info(&self) -> AppInfo {
        AppInfo {
            name: self.config.get_or("APP_NAME", "mono-ws-chat"),
            description: "Monolith WebSocket chat demo built with Ferrite + ferrite-ws + htmx + Tailwind CDN.",
            features: &[
                "auth-jwt",
                "throttler",
                "swagger-ui",
                "health-indicators",
                "websocket-chat",
            ],
        }
    }
}
