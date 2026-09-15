use ferrite_auth_jwt::AuthModule;
use ferrite_cache_redis::CacheModule;
use ferrite_cqrs::CqrsModuleImpl;
use ferrite_framework::module;
use ferrite_grpc::GrpcModuleImpl;
use ferrite_kafka::KafkaModuleImpl;
use ferrite_swagger::SwaggerModule;
use ferrite_throttler::ThrottlerModule;

use crate::app_service::AppService;
use crate::checkout::controller::CheckoutController;
use crate::checkout::service::CheckoutService;
use crate::modules::health::{HealthController, RootController};
use crate::orders::controller::OrdersController;
use crate::orders::handlers::{
    AuditOrderEventsHandler, CancelOrderHandler, CreateOrderHandler, GetOrderHandler,
    KafkaOrderPublisher, ListOrdersHandler, OrderCreatedEventHandler,
    OrderStatusChangedEventHandler, UpdateOrderStatusHandler,
};
use crate::orders::repo::{OrderItemsRepo, OrdersRepo};
use crate::orders::service::OrdersService;

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
        OrdersController,
        CheckoutController,
    ],
    providers = [
        AppService,
        OrdersService,
        OrdersRepo,
        OrderItemsRepo,
        CreateOrderHandler,
        UpdateOrderStatusHandler,
        CancelOrderHandler,
        GetOrderHandler,
        ListOrdersHandler,
        OrderCreatedEventHandler,
        OrderStatusChangedEventHandler,
        AuditOrderEventsHandler,
        KafkaOrderPublisher,
        CheckoutService,
    ],
)]
pub struct AppModule;
