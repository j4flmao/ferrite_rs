use ferrite_auth_jwt::AuthModule;
use ferrite_framework::module;
use ferrite_health::HealthModule;
use ferrite_swagger::SwaggerModule;
use ferrite_throttler::ThrottlerModule;
use ferrite_ws::WsModuleImpl;

use crate::app_service::AppService;
use crate::modules::auth::{AuthController, AuthService, UsersRepo};
use crate::modules::chat::{ChatController, ChatGateway, ChatService};
use crate::modules::health::RootController;
use crate::modules::homepage::HomePageController;

#[module(
    imports = [AuthModule, HealthModule, SwaggerModule, ThrottlerModule, WsModuleImpl],
    controllers = [RootController, HomePageController, AuthController, ChatController],
    providers = [AppService, UsersRepo, AuthService, ChatService, ChatGateway],
)]
pub struct AppModule;
