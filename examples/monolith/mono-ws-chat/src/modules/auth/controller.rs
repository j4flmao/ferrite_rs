use super::dto::{AuthResponse, LoginDto, MeResponse, RegisterDto};
use super::service::AuthService;
use ferrite_auth_jwt::{AuthGuard, CurrentUser};
use ferrite_framework::{controller, impl_controller, inject, HttpError, Json};

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
        Ok(Json(self.service.register(dto)?))
    }

    #[post("/login")]
    pub async fn login(&self, Json(dto): Json<LoginDto>) -> Result<Json<AuthResponse>, HttpError> {
        Ok(Json(self.service.login(dto)?))
    }

    #[get("/me")]
    #[use_guards(AuthGuard)]
    pub async fn me(
        &self,
        CurrentUser(claims): CurrentUser,
    ) -> Result<Json<MeResponse>, HttpError> {
        Ok(Json(self.service.me(claims.sub, &claims.email)?))
    }
}
