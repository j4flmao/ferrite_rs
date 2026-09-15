use ferrite_auth_jwt::AuthModule;
use ferrite_cache_redis::CacheModule;
use ferrite_cqrs::CqrsModuleImpl;
use ferrite_framework::module;
use ferrite_grpc::GrpcModuleImpl;
use ferrite_kafka::KafkaModuleImpl;
use ferrite_swagger::SwaggerModule;
use ferrite_throttler::ThrottlerModule;

use crate::app_service::AppService;
use crate::carts::controllers::{CartsController, ItemsController};
use crate::carts::handlers::{
    AddToCartHandler, AuditCartEventsHandler, CartCreatedEventHandler, CartUpdatedEventHandler,
    CartsEventsKafkaHandler, ClearCartHandler, ConvertCartHandler, CreateCartHandler,
    GetCartHandler, GetSessionCartHandler, GetUserCartHandler, KafkaCartPublisher,
    ListCartsHandler, RemoveFromCartHandler, UpdateCartItemHandler,
};
use crate::carts::repo::CartsRepo;
use crate::carts::service::CartsService;
use crate::modules::health::{HealthController, RootController};

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
        CartsController,
        ItemsController,
    ],
    providers = [
        AppService,
        CartsService,
        CartsRepo,
        CreateCartHandler,
        AddToCartHandler,
        UpdateCartItemHandler,
        RemoveFromCartHandler,
        ClearCartHandler,
        ConvertCartHandler,
        GetCartHandler,
        GetUserCartHandler,
        GetSessionCartHandler,
        ListCartsHandler,
        CartCreatedEventHandler,
        CartUpdatedEventHandler,
        AuditCartEventsHandler,
        KafkaCartPublisher,
        CartsEventsKafkaHandler,
    ],
)]
pub struct AppModule;
