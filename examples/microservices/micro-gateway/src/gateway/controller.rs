use axum::{
    body::Body,
    extract::Path,
    http::{HeaderMap, Method, Request, Uri},
    response::{IntoResponse, Response},
};
use ferrite_auth_jwt::AuthGuard;
use ferrite_framework::{controller, impl_controller, inject, Json};

use crate::gateway::dto::GatewayServicesResponse;
use crate::gateway::service::ProxyService;

#[controller("/")]
pub struct GatewayController {
    proxy: ProxyService,
}

#[impl_controller]
impl GatewayController {
    #[inject]
    pub fn new(proxy: ProxyService) -> Self {
        Self { proxy }
    }

    #[get("/services")]
    pub async fn services(&self) -> Json<GatewayServicesResponse> {
        Json(self.proxy.services_status().await)
    }

    #[get("/auth/{*path}")]
    pub async fn proxy_auth_get(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/auth/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::GET, headers, uri, full_path, request)
            .await
    }

    #[post("/auth/{*path}")]
    pub async fn proxy_auth_post(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/auth/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::POST, headers, uri, full_path, request)
            .await
    }

    #[put("/auth/{*path}")]
    pub async fn proxy_auth_put(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/auth/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::PUT, headers, uri, full_path, request)
            .await
    }

    #[delete("/auth/{*path}")]
    pub async fn proxy_auth_delete(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/auth/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::DELETE, headers, uri, full_path, request)
            .await
    }

    #[patch("/auth/{*path}")]
    pub async fn proxy_auth_patch(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/auth/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::PATCH, headers, uri, full_path, request)
            .await
    }

    #[get("/auth")]
    pub async fn proxy_auth_root_get(
        &self,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        self.do_proxy(Method::GET, headers, uri, "/auth".into(), request)
            .await
    }

    #[post("/auth")]
    pub async fn proxy_auth_root_post(
        &self,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        self.do_proxy(Method::POST, headers, uri, "/auth".into(), request)
            .await
    }

    #[get("/users/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_users_get(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/users/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::GET, headers, uri, full_path, request)
            .await
    }

    #[post("/users/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_users_post(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/users/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::POST, headers, uri, full_path, request)
            .await
    }

    #[put("/users/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_users_put(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/users/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::PUT, headers, uri, full_path, request)
            .await
    }

    #[delete("/users/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_users_delete(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/users/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::DELETE, headers, uri, full_path, request)
            .await
    }

    #[patch("/users/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_users_patch(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/users/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::PATCH, headers, uri, full_path, request)
            .await
    }

    #[get("/users")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_users_root_get(
        &self,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        self.do_proxy(Method::GET, headers, uri, "/users".into(), request)
            .await
    }

    #[post("/users")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_users_root_post(
        &self,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        self.do_proxy(Method::POST, headers, uri, "/users".into(), request)
            .await
    }

    #[get("/products/{*path}")]
    pub async fn proxy_products_get(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/products/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::GET, headers, uri, full_path, request)
            .await
    }

    #[post("/products/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_products_post(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/products/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::POST, headers, uri, full_path, request)
            .await
    }

    #[put("/products/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_products_put(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/products/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::PUT, headers, uri, full_path, request)
            .await
    }

    #[delete("/products/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_products_delete(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/products/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::DELETE, headers, uri, full_path, request)
            .await
    }

    #[patch("/products/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_products_patch(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/products/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::PATCH, headers, uri, full_path, request)
            .await
    }

    #[get("/products")]
    pub async fn proxy_products_root_get(
        &self,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        self.do_proxy(Method::GET, headers, uri, "/products".into(), request)
            .await
    }

    #[post("/products")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_products_root_post(
        &self,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        self.do_proxy(Method::POST, headers, uri, "/products".into(), request)
            .await
    }

    #[get("/orders/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_orders_get(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/orders/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::GET, headers, uri, full_path, request)
            .await
    }

    #[post("/orders/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_orders_post(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/orders/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::POST, headers, uri, full_path, request)
            .await
    }

    #[put("/orders/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_orders_put(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/orders/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::PUT, headers, uri, full_path, request)
            .await
    }

    #[delete("/orders/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_orders_delete(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/orders/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::DELETE, headers, uri, full_path, request)
            .await
    }

    #[patch("/orders/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_orders_patch(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/orders/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::PATCH, headers, uri, full_path, request)
            .await
    }

    #[get("/orders")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_orders_root_get(
        &self,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        self.do_proxy(Method::GET, headers, uri, "/orders".into(), request)
            .await
    }

    #[post("/orders")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_orders_root_post(
        &self,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        self.do_proxy(Method::POST, headers, uri, "/orders".into(), request)
            .await
    }

    #[get("/carts/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_carts_get(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/carts/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::GET, headers, uri, full_path, request)
            .await
    }

    #[post("/carts/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_carts_post(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/carts/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::POST, headers, uri, full_path, request)
            .await
    }

    #[put("/carts/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_carts_put(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/carts/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::PUT, headers, uri, full_path, request)
            .await
    }

    #[delete("/carts/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_carts_delete(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/carts/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::DELETE, headers, uri, full_path, request)
            .await
    }

    #[patch("/carts/{*path}")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_carts_patch(
        &self,
        Path(path): Path<String>,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        let full_path = format!("/carts/{}", path.trim_start_matches('/'));
        self.do_proxy(Method::PATCH, headers, uri, full_path, request)
            .await
    }

    #[get("/carts")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_carts_root_get(
        &self,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        self.do_proxy(Method::GET, headers, uri, "/carts".into(), request)
            .await
    }

    #[post("/carts")]
    #[use_guards(AuthGuard)]
    pub async fn proxy_carts_root_post(
        &self,
        headers: HeaderMap,
        uri: Uri,
        request: Request<Body>,
    ) -> Response<Body> {
        self.do_proxy(Method::POST, headers, uri, "/carts".into(), request)
            .await
    }

    async fn do_proxy(
        &self,
        method: Method,
        headers: HeaderMap,
        uri: Uri,
        path: String,
        request: Request<Body>,
    ) -> Response<Body> {
        let (_, body) = request.into_parts();
        let bytes = match axum::body::to_bytes(body, usize::MAX).await {
            Ok(b) => b.to_vec(),
            Err(e) => {
                return (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("body read error: {e}"),
                )
                    .into_response();
            }
        };
        match self
            .proxy
            .proxy_request(method, uri, headers, path, bytes)
            .await
        {
            Ok(r) => r,
            Err(e) => e.into_response(),
        }
    }
}
