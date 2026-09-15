use crate::checkout::dto::{CheckoutDto, CheckoutSession};
use crate::checkout::service::CheckoutService;
use ferrite_auth_jwt::{AuthGuard, CurrentUser};
use ferrite_framework::{controller, impl_controller, inject, HttpError, Json};

#[controller("/checkout")]
pub struct CheckoutController {
    service: CheckoutService,
}

#[impl_controller]
impl CheckoutController {
    #[inject]
    pub fn new(service: CheckoutService) -> Self {
        Self { service }
    }

    #[post("/session")]
    #[use_guards(AuthGuard)]
    pub async fn create_session(
        &self,
        CurrentUser(claims): CurrentUser,
        Json(body): Json<CheckoutDto>,
    ) -> Result<Json<CheckoutSession>, HttpError> {
        Ok(Json(self.service.create_session(body, claims.sub).await?))
    }
}
