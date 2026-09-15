#[ferrite_framework::bootstrap]
async fn main() {
    let app = ferrite_framework::Ferrite::create::<AppModule>().await;

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    let addr = format!("0.0.0.0:{port}");
    eprintln!("[ferrite-http] micro-gateway listening on {addr}");
    app.listen(&addr).await.expect("failed to bind port");
}

mod app_module;
mod app_service;
mod gateway;
mod modules;
mod swagger_schemas;

use inventory as _;

use app_module::AppModule;
