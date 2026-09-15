use ferrite_auth_jwt::{AuthGuard, CurrentUser};
use ferrite_framework::{controller, impl_controller, inject, HttpError, Json, Path};

use super::dto::{UpdateUserDto, UserResponse};
use super::UsersService;

#[controller("/users")]
pub struct UsersController {
    service: UsersService,
}

#[impl_controller]
impl UsersController {
    #[inject]
    pub fn new(service: UsersService) -> Self {
        Self { service }
    }

    #[use_guards(AuthGuard)]
    #[get("/me")]
    pub async fn me(
        &self,
        CurrentUser(claims): CurrentUser,
    ) -> Result<Json<UserResponse>, HttpError> {
        let user = self
            .service
            .find_one(claims.sub)
            .await
            .ok_or_else(|| HttpError::not_found("user"))?;
        Ok(Json(user.into()))
    }

    #[use_guards(AuthGuard)]
    #[get("/")]
    pub async fn find_all(&self) -> Json<Vec<UserResponse>> {
        let list = self.service.find_all().await;
        Json(list.into_iter().map(Into::into).collect())
    }

    #[use_guards(AuthGuard)]
    #[get("/{id}")]
    pub async fn find_one(&self, Path(id): Path<i64>) -> Result<Json<UserResponse>, HttpError> {
        let u = self
            .service
            .find_one(id)
            .await
            .ok_or_else(|| HttpError::not_found("user"))?;
        Ok(Json(u.into()))
    }

    #[use_guards(AuthGuard)]
    #[patch("/{id}")]
    pub async fn update(
        &self,
        Path(id): Path<i64>,
        Json(dto): Json<UpdateUserDto>,
    ) -> Result<Json<UserResponse>, HttpError> {
        let u = self
            .service
            .update(id, dto.name, dto.role)
            .await
            .ok_or_else(|| HttpError::not_found("user"))?;
        Ok(Json(u.into()))
    }

    #[use_guards(AuthGuard)]
    #[delete("/{id}")]
    pub async fn delete(&self, Path(id): Path<i64>) -> Json<&'static str> {
        if self.service.delete(id).await {
            Json("deleted")
        } else {
            Json("not found")
        }
    }
}
