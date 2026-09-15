#[ferrite_framework::bootstrap]
async fn main() {
    let app = ferrite_framework::Ferrite::create::<AppModule>().await;
    let port = std::env::var("PORT").unwrap_or_else(|_| "3002".into());
    let addr = format!("0.0.0.0:{port}");
    app.listen(&addr).await.expect("failed to bind port");
}

mod app_module;
mod app_service;
mod auth;
mod carts;
mod categories;
mod modules;
mod orders;
mod products;
mod swagger_schemas;
mod users;

use ferrite_orm_sea as _;

use app_module::AppModule;
