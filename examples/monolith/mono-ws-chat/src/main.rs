#[ferrite_framework::bootstrap]
async fn main() {
    let mut app = ferrite_framework::Ferrite::create::<AppModule>().await;
    let port = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(3003);
    ferrite_ws::mount_on(&mut app.router, app.container.clone(), "/ws");
    app.listen(&format!("0.0.0.0:{port}"))
        .await
        .expect("failed to bind port");
}

mod app_module;
mod app_service;
mod modules;
mod swagger_schemas;

use app_module::AppModule;
