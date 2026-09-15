use ferrite_framework::{controller, impl_controller, inject, HttpError, Json};

use super::auth_service::AuthService;
use super::dto::{AuthResponse, LoginDto, RegisterDto};

#[controller("/auth")]
pub struct AuthController {
    service: AuthService,
}

#[impl_controller]
impl AuthController {
    #[inject]
    pub fn new(service: AuthService) -> Self {
        Self { service }
    }

    #[post("/register")]
    pub async fn register(
        &self,
        Json(dto): Json<RegisterDto>,
    ) -> Result<Json<AuthResponse>, HttpError> {
        Ok(Json(self.service.register(dto).await?))
    }

    #[post("/login")]
    pub async fn login(&self, Json(dto): Json<LoginDto>) -> Result<Json<AuthResponse>, HttpError> {
        Ok(Json(self.service.login(dto).await?))
    }
}
