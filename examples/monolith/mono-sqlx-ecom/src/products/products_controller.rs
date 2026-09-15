use ferrite_auth_jwt::AuthGuard;
use ferrite_framework::{controller, impl_controller, inject, HttpError, Json, Path, Query};
use serde::Deserialize;

use super::dto::{CreateProductDto, ProductResponse, UpdateProductDto};
use super::products_service::ProductsService;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub category: Option<i64>,
}

#[controller("/products")]
pub struct ProductsController {
    service: ProductsService,
}

#[impl_controller]
impl ProductsController {
    #[inject]
    pub fn new(service: ProductsService) -> Self {
        Self { service }
    }

    #[get("/")]
    pub async fn find_all(&self, Query(q): Query<ListQuery>) -> Json<Vec<ProductResponse>> {
        let list = match q.category {
            Some(cid) => self.service.find_by_category(cid).await,
            None => self.service.find_all().await,
        };
        Json(list.into_iter().map(Into::into).collect())
    }

    #[get("/{id}")]
    pub async fn find_one(&self, Path(id): Path<i64>) -> Result<Json<ProductResponse>, HttpError> {
        let p = self
            .service
            .find_one(id)
            .await
            .ok_or_else(|| HttpError::not_found("product"))?;
        Ok(Json(p.into()))
    }

    #[use_guards(AuthGuard)]
    #[post("/")]
    pub async fn create(&self, Json(dto): Json<CreateProductDto>) -> Json<ProductResponse> {
        Json(
            self.service
                .create(
                    dto.name,
                    dto.slug,
                    dto.description,
                    dto.price_cents,
                    dto.stock,
                    dto.category_id,
                    dto.images,
                )
                .await
                .into(),
        )
    }

    #[use_guards(AuthGuard)]
    #[patch("/{id}")]
    pub async fn update(
        &self,
        Path(id): Path<i64>,
        Json(dto): Json<UpdateProductDto>,
    ) -> Result<Json<ProductResponse>, HttpError> {
        let updated = self
            .service
            .update(
                id,
                dto.name,
                dto.slug,
                dto.description,
                dto.price_cents,
                dto.stock,
                dto.category_id.map(Some),
                dto.images,
            )
            .await
            .ok_or_else(|| HttpError::not_found("product"))?;
        Ok(Json(updated.into()))
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
