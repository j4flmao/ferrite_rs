use ferrite_auth_jwt::{AuthGuard, CurrentUser};
use ferrite_framework::{controller, impl_controller, inject, HttpError, Json, Path};

use super::dto::CheckoutDto;
use super::models::Order;
use super::OrdersService;
use crate::users::UsersService;

#[controller("/orders")]
pub struct OrdersController {
    service: OrdersService,
    users: UsersService,
}

#[impl_controller]
impl OrdersController {
    #[inject]
    pub fn new(service: OrdersService, users: UsersService) -> Self {
        Self { service, users }
    }

    async fn is_admin(&self, user_id: i64) -> bool {
        self.users
            .find_one(user_id)
            .await
            .map(|u| u.role == "admin")
            .unwrap_or(false)
    }

    #[use_guards(AuthGuard)]
    #[post("/checkout")]
    pub async fn checkout(
        &self,
        CurrentUser(claims): CurrentUser,
        Json(dto): Json<CheckoutDto>,
    ) -> Result<Json<Order>, HttpError> {
        let order = self
            .service
            .checkout(claims.sub, dto.shipping_address, dto.notes)
            .await
            .map_err(HttpError::bad_request)?;
        Ok(Json(order))
    }

    #[use_guards(AuthGuard)]
    #[get("/")]
    pub async fn find_all(&self, CurrentUser(claims): CurrentUser) -> Json<Vec<Order>> {
        let admin = self.is_admin(claims.sub).await;
        Json(self.service.find_all_for_user(claims.sub, admin).await)
    }

    #[use_guards(AuthGuard)]
    #[get("/{id}")]
    pub async fn find_one(
        &self,
        CurrentUser(claims): CurrentUser,
        Path(id): Path<i64>,
    ) -> Result<Json<Order>, HttpError> {
        let admin = self.is_admin(claims.sub).await;
        let order = self
            .service
            .find_one_for_user(claims.sub, id, admin)
            .await
            .ok_or_else(|| HttpError::not_found("order"))?;
        Ok(Json(order))
    }
}
