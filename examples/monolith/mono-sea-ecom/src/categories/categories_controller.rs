use ferrite_auth_jwt::AuthGuard;
use ferrite_framework::{controller, impl_controller, inject, HttpError, Json, Path};

use super::categories_service::CategoriesService;
use super::dto::{CategoryResponse, CreateCategoryDto, UpdateCategoryDto};

#[controller("/categories")]
pub struct CategoriesController {
    service: CategoriesService,
}

#[impl_controller]
impl CategoriesController {
    #[inject]
    pub fn new(service: CategoriesService) -> Self {
        Self { service }
    }

    #[get("/")]
    pub async fn find_all(&self) -> Json<Vec<CategoryResponse>> {
        let list = self.service.find_all().await;
        Json(list.into_iter().map(Into::into).collect())
    }

    #[get("/{id}")]
    pub async fn find_one(&self, Path(id): Path<i64>) -> Result<Json<CategoryResponse>, HttpError> {
        let c = self
            .service
            .find_one(id)
            .await
            .ok_or_else(|| HttpError::not_found("category"))?;
        Ok(Json(c.into()))
    }

    #[use_guards(AuthGuard)]
    #[post("/")]
    pub async fn create(
        &self,
        Json(dto): Json<CreateCategoryDto>,
    ) -> Result<Json<CategoryResponse>, HttpError> {
        let created = self
            .service
            .create(dto.name, dto.slug, dto.description, dto.parent_id)
            .await
            .map_err(HttpError::bad_request)?;
        Ok(Json(created.into()))
    }

    #[use_guards(AuthGuard)]
    #[patch("/{id}")]
    pub async fn update(
        &self,
        Path(id): Path<i64>,
        Json(dto): Json<UpdateCategoryDto>,
    ) -> Result<Json<CategoryResponse>, HttpError> {
        let updated = self
            .service
            .update(
                id,
                dto.name,
                dto.slug,
                dto.description.map(Some),
                dto.parent_id.map(Some),
            )
            .await
            .map_err(HttpError::bad_request)?;
        Ok(Json(updated.into()))
    }

    #[use_guards(AuthGuard)]
    #[delete("/{id}")]
    pub async fn delete(&self, Path(id): Path<i64>) -> Result<Json<&'static str>, HttpError> {
        let deleted = self
            .service
            .delete(id)
            .await
            .map_err(HttpError::bad_request)?;
        if deleted {
            Ok(Json("deleted"))
        } else {
            Err(HttpError::not_found("category"))
        }
    }
}
