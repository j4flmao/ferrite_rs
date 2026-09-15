use ferrite_auth_jwt::AuthModule;
use ferrite_cache_redis::CacheModule;
use ferrite_cqrs::CqrsModuleImpl;
use ferrite_framework::module;
use ferrite_grpc::GrpcModuleImpl;
use ferrite_kafka::KafkaModuleImpl;
use ferrite_swagger::SwaggerModule;
use ferrite_throttler::ThrottlerModule;

use crate::app_service::AppService;
use crate::auth::handlers::{
    AuditAuthEventsHandler, AuthEventsKafkaHandler, GetUserQueryHandler, KafkaAuthPublisher,
    LoginUserHandler, RegisterUserHandler, UserCreatedEventHandler, ValidateTokenQueryHandler,
};
use crate::auth::{AuthController, AuthService, UsersRepo};
use crate::modules::health::{HealthController, RootController};
use crate::tokens::TokensController;

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
        TokensController,
    ],
    providers = [
        AppService,
        AuthService,
        UsersRepo,
        RegisterUserHandler,
        LoginUserHandler,
        GetUserQueryHandler,
        ValidateTokenQueryHandler,
        UserCreatedEventHandler,
        AuditAuthEventsHandler,
        KafkaAuthPublisher,
        AuthEventsKafkaHandler,
    ],
)]
pub struct AppModule;
