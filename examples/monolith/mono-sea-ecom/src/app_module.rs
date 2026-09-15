use ferrite_auth_jwt::AuthModule;
use ferrite_cache_redis::CacheModule;
use ferrite_framework::module;
use ferrite_health::HealthModule;
use ferrite_kafka::KafkaModuleImpl;
use ferrite_swagger::SwaggerModule;
use ferrite_throttler::ThrottlerModule;

use crate::app_service::AppService;
use crate::auth::{AuthController, AuthService};
use crate::carts::{CartsController, CartsService};
use crate::categories::{CategoriesController, CategoriesService};
use crate::modules::health::RootController;
use crate::orders::{OrdersController, OrdersService};
use crate::products::{ProductsController, ProductsService};
use crate::users::{UsersController, UsersService};

#[module(
    imports = [
        AuthModule,
        HealthModule,
        SwaggerModule,
        ThrottlerModule,
        CacheModule,
        KafkaModuleImpl,
    ],
    controllers = [
        RootController,
        AuthController,
        UsersController,
        CategoriesController,
        ProductsController,
        CartsController,
        OrdersController,
    ],
    providers = [
        AppService,
        AuthService,
        UsersService,
        CategoriesService,
        ProductsService,
        CartsService,
        OrdersService,
    ],
)]
pub struct AppModule;
