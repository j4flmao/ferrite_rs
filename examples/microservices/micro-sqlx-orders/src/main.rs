#[ferrite_framework::bootstrap]
async fn main() {
    let app = ferrite_framework::Ferrite::create::<AppModule>().await;

    let grpc = app.container.get::<ferrite_grpc::GrpcServer>();
    grpc.add_service(crate::orders::OrdersGrpcService::new());
    let grpc_names = grpc.service_names();
    let manual_services = grpc.manual_len();
    tokio::spawn(async move {
        let grpc_addr = std::env::var("GRPC_ADDR").unwrap_or_else(|_| "0.0.0.0:50054".into());
        eprintln!(
            "[ferrite-grpc] orders.grpc = {:?} (inventory={grpc_names:?}, manual_added={manual_services}) listening on {grpc_addr}",
            crate::orders::ORDERS_GRPC_SERVICE_NAME
        );
        let _ = grpc.build_server();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    });

    let kafka = app.container.get::<ferrite_kafka::KafkaServer>();
    tokio::spawn(async move {
        kafka.run().await;
    });

    let port = std::env::var("PORT").unwrap_or_else(|_| "3007".into());
    let addr = format!("0.0.0.0:{port}");
    eprintln!("[ferrite-http] listening on {addr}");
    app.listen(&addr).await.expect("failed to bind port");
}

mod app_module;
mod app_service;
mod checkout;
mod modules;
mod orders;
mod swagger_schemas;

use ferrite_cqrs as _;
use ferrite_grpc as _;
use ferrite_kafka as _;
use ferrite_orm_sqlx as _;
use inventory as _;

use app_module::AppModule;
