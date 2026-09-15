use ferrite_auth_jwt::AuthModule;
use ferrite_cache_redis::CacheModule;
use ferrite_framework::module;
use ferrite_swagger::SwaggerModule;
use ferrite_throttler::ThrottlerModule;

use crate::app_service::AppService;
use crate::gateway::{GatewayController, ProxyService};
use crate::modules::health::{HealthController, RootController};

#[module(
    imports = [
        AuthModule,
        SwaggerModule,
        ThrottlerModule,
        CacheModule,
    ],
    controllers = [
        RootController,
        HealthController,
        GatewayController,
    ],
    providers = [
        AppService,
        ProxyService,
    ],
)]
pub struct AppModule;
