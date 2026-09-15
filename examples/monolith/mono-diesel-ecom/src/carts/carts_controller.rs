use ferrite_auth_jwt::{AuthGuard, CurrentUser};
use ferrite_framework::{controller, impl_controller, inject, HttpError, Json, Path};

use super::dto::{AddItemDto, UpdateQtyDto};
use super::models::CartSummary;
use super::CartsService;

#[controller("/cart")]
pub struct CartsController {
    service: CartsService,
}

#[impl_controller]
impl CartsController {
    #[inject]
    pub fn new(service: CartsService) -> Self {
        Self { service }
    }

    #[use_guards(AuthGuard)]
    #[get("/")]
    pub async fn get(&self, CurrentUser(claims): CurrentUser) -> Json<CartSummary> {
        Json(self.service.summary(claims.sub).await)
    }

    #[use_guards(AuthGuard)]
    #[post("/items")]
    pub async fn add_item(
        &self,
        CurrentUser(claims): CurrentUser,
        Json(dto): Json<AddItemDto>,
    ) -> Result<Json<CartSummary>, HttpError> {
        let summary = self
            .service
            .add_item(claims.sub, dto.product_id, dto.quantity)
            .await
            .map_err(HttpError::bad_request)?;
        Ok(Json(summary))
    }

    #[use_guards(AuthGuard)]
    #[patch("/items/{product_id}")]
    pub async fn update_qty(
        &self,
        CurrentUser(claims): CurrentUser,
        Path(product_id): Path<i64>,
        Json(dto): Json<UpdateQtyDto>,
    ) -> Result<Json<CartSummary>, HttpError> {
        let summary = self
            .service
            .update_quantity(claims.sub, product_id, dto.quantity)
            .await
            .map_err(HttpError::bad_request)?;
        Ok(Json(summary))
    }

    #[use_guards(AuthGuard)]
    #[delete("/items/{product_id}")]
    pub async fn remove_item(
        &self,
        CurrentUser(claims): CurrentUser,
        Path(product_id): Path<i64>,
    ) -> Json<CartSummary> {
        Json(self.service.remove_item(claims.sub, product_id).await)
    }

    #[use_guards(AuthGuard)]
    #[delete("/")]
    pub async fn clear(&self, CurrentUser(claims): CurrentUser) -> Json<CartSummary> {
        self.service.clear(claims.sub).await;
        Json(self.service.summary(claims.sub).await)
    }
}
