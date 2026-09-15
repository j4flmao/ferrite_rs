use ferrite_auth_jwt::AuthModule;
use ferrite_cache_redis::CacheModule;
use ferrite_cqrs::CqrsModuleImpl;
use ferrite_framework::module;
use ferrite_grpc::GrpcModuleImpl;
use ferrite_kafka::KafkaModuleImpl;
use ferrite_swagger::SwaggerModule;
use ferrite_throttler::ThrottlerModule;

use crate::app_service::AppService;
use crate::auth::{AuthController, AuthService, UsersRepo};
use crate::modules::health::{HealthController, RootController};
use crate::products::controllers::ProductsController;
use crate::products::handlers::{
    AuditProductEventsHandler, CreateProductHandler, GetProductHandler, KafkaProductPublisher,
    ListProductsHandler, ProductCreatedEventHandler, UpdateProductStockHandler,
};
use crate::products::repo::ProductsRepo;

#[module(
    imports = [
        AuthModule,
        SwaggerModule,
        ThrottlerModule,
        CacheModule,
        KafkaModuleImpl,
        CqrsModuleImpl,
        GrpcModuleImpl,
    ],
    controllers = [
        RootController,
        HealthController,
        AuthController,
        ProductsController,
    ],
    providers = [
        AppService,
        AuthService,
        UsersRepo,
        ProductsRepo,
        CreateProductHandler,
        UpdateProductStockHandler,
        GetProductHandler,
        ListProductsHandler,
        ProductCreatedEventHandler,
        AuditProductEventsHandler,
        KafkaProductPublisher,
    ],
)]
pub struct AppModule;
