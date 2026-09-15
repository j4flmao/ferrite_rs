use crate::auth::dto::{AuthResponse, LoginDto, RegisterDto, User};
use crate::auth::service::AuthService;
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
        Json(body): Json<RegisterDto>,
    ) -> Result<Json<AuthResponse>, HttpError> {
        Ok(Json(self.service.register(body).await?))
    }

    #[post("/login")]
    pub async fn login(&self, Json(body): Json<LoginDto>) -> Result<Json<AuthResponse>, HttpError> {
        Ok(Json(self.service.login(body).await?))
    }

    #[get("/me")]
    #[use_guards(AuthGuard)]
    pub async fn me(&self, CurrentUser(claims): CurrentUser) -> Json<Option<User>> {
        Json(self.service.me(claims.sub))
    }
}
