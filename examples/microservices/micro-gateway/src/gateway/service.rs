use std::time::Duration;

use axum::{
    body::Body,
    http::{HeaderMap, Method, StatusCode, Uri},
    response::Response,
};
use ferrite_framework::{inject, injectable, HttpError};
use reqwest::Client;

use crate::gateway::dto::{GatewayServicesResponse, ServiceStatus};

#[injectable]
#[derive(Debug, Clone)]
pub struct ProxyService {
    client: Client,
}

impl ProxyService {
    #[inject]
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create reqwest client");
        Self {
            client: ::std::sync::Arc::new(client),
        }
    }

    fn route_map(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("/auth", "AUTH_SERVICE_URL"),
            ("/users", "USER_SERVICE_URL"),
            ("/products", "PRODUCT_SERVICE_URL"),
            ("/orders", "ORDER_SERVICE_URL"),
            ("/carts", "CART_SERVICE_URL"),
        ]
    }

    pub fn resolve_backend(&self, path: &str) -> Option<String> {
        for (prefix, env_key) in self.route_map() {
            if path.starts_with(prefix) {
                let base = std::env::var(env_key).ok()?;
                let base = base.trim_end_matches('/');
                let target = format!("{base}{path}");
                return Some(target);
            }
        }
        None
    }

    pub async fn proxy_request(
        &self,
        method: Method,
        uri: Uri,
        headers: HeaderMap,
        path: String,
        body: Vec<u8>,
    ) -> Result<Response<Body>, HttpError> {
        let target_uri = self
            .resolve_backend(&path)
            .ok_or_else(|| HttpError::not_found("no backend route for this path"))?;

        let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
        let final_url = if target_uri.contains('?') {
            format!("{target_uri}&{}", query.trim_start_matches('?'))
        } else {
            format!("{target_uri}{query}")
        };

        let mut req_builder = self.client.request(method, &final_url).body(body);

        for (key, value) in headers.iter() {
            let name_str = key.as_str();
            if matches!(
                name_str.to_ascii_lowercase().as_str(),
                "host" | "content-length"
            ) {
                continue;
            }
            req_builder = req_builder.header(key, value);
        }

        let upstream = req_builder.send().await.map_err(map_reqwest_err)?;

        let mut builder = Response::builder().status(upstream.status());

        for (key, value) in upstream.headers().iter() {
            let name_str = key.as_str();
            if matches!(
                name_str.to_ascii_lowercase().as_str(),
                "content-length" | "transfer-encoding" | "connection" | "keep-alive"
            ) {
                continue;
            }
            builder = builder.header(key, value);
        }

        let bytes = upstream
            .bytes()
            .await
            .map_err(|e| HttpError::internal(format!("upstream body error: {e}")))?;

        builder
            .body(Body::from(bytes))
            .map_err(|e| HttpError::internal(format!("response build error: {e}")))
    }

    pub async fn services_status(&self) -> GatewayServicesResponse {
        let routes = vec![
            ("Auth Service", "AUTH_SERVICE_URL", "http://localhost:3005"),
            ("User Service", "USER_SERVICE_URL", "http://localhost:3006"),
            (
                "Product Service",
                "PRODUCT_SERVICE_URL",
                "http://localhost:3004",
            ),
            (
                "Order Service",
                "ORDER_SERVICE_URL",
                "http://localhost:3007",
            ),
            ("Cart Service", "CART_SERVICE_URL", "http://localhost:3008"),
        ];

        let mut services = Vec::with_capacity(routes.len());
        for (name, env_key, default_url) in routes {
            let url = std::env::var(env_key).unwrap_or_else(|_| default_url.into());
            let status = self.check_service(&url).await;
            services.push(ServiceStatus {
                name: name.into(),
                url,
                status,
            });
        }

        GatewayServicesResponse {
            services,
            gateway: "micro-gateway".into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    async fn check_service(&self, url: &str) -> String {
        let health_url = format!("{}/healthz", url.trim_end_matches('/'));
        let timeout = Duration::from_secs(3);
        let fut = self.client.get(&health_url).timeout(timeout).send();
        match tokio::time::timeout(timeout, fut).await {
            Ok(Ok(r)) if r.status().is_success() => "ok".into(),
            Ok(Ok(_)) => "unhealthy".into(),
            Ok(Err(_)) | Err(_) => "unreachable".into(),
        }
    }
}

fn map_reqwest_err(e: reqwest::Error) -> HttpError {
    if e.is_timeout() {
        return HttpError::new(StatusCode::GATEWAY_TIMEOUT, "gateway timeout");
    }
    if e.is_connect() {
        return HttpError::new(StatusCode::BAD_GATEWAY, "bad gateway: connection failed");
    }
    if e.is_request() || e.is_body() || e.is_decode() {
        return HttpError::new(
            StatusCode::BAD_GATEWAY,
            "bad gateway: upstream request failed",
        );
    }
    HttpError::internal(format!("upstream error: {e}"))
}
