use ferrite_auth_jwt::AuthModule;
use ferrite_cache_redis::CacheModule;
use ferrite_cqrs::CqrsModuleImpl;
use ferrite_framework::module;
use ferrite_grpc::GrpcModuleImpl;
use ferrite_kafka::KafkaModuleImpl;
use ferrite_swagger::SwaggerModule;
use ferrite_throttler::ThrottlerModule;

use crate::app_service::AppService;
use crate::modules::health::{HealthController, RootController};
use crate::profiles::controller::ProfilesController;
use crate::users::controller::UsersController;
use crate::users::handlers::{
    AuditUserEventsHandler, ChangePasswordHandler, CreateUserHandler, DeleteUserHandler,
    GetUserHandler, KafkaUserPublisher, ListUsersHandler, UpdateUserHandler,
    UserCreatedEventHandler, UserUpdatedEventHandler,
};
use crate::users::repo::UsersRepo;
use crate::users::service::UsersService;

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
        UsersController,
        ProfilesController,
    ],
    providers = [
        AppService,
        UsersService,
        UsersRepo,
        CreateUserHandler,
        UpdateUserHandler,
        DeleteUserHandler,
        ChangePasswordHandler,
        GetUserHandler,
        ListUsersHandler,
        UserCreatedEventHandler,
        UserUpdatedEventHandler,
        AuditUserEventsHandler,
        KafkaUserPublisher,
    ],
)]
pub struct AppModule;
