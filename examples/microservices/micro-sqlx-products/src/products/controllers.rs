use crate::products::dto::{
    CreateProductCommand, CreateProductDto, GetProductQuery, ListProductsQuery, Product,
    ProductList, UpdateProductStockCommand, UpdateStockDto,
};
use crate::products::repo::ProductsRepo;
use ferrite_auth_jwt::AuthGuard;
use ferrite_cqrs::{CommandBus, EventBus, QueryBus};
use ferrite_framework::{controller, impl_controller, inject, HttpError, Json, Path, Query};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[controller("/products")]
pub struct ProductsController {
    commands: CommandBus,
    queries: QueryBus,
    events: EventBus,
    repo: ProductsRepo,
}

#[impl_controller]
impl ProductsController {
    #[inject]
    pub fn new(
        commands: CommandBus,
        queries: QueryBus,
        events: EventBus,
        repo: ProductsRepo,
    ) -> Self {
        Self {
            commands,
            queries,
            events,
            repo,
        }
    }

    #[post("/commands/create")]
    #[use_guards(AuthGuard)]
    pub async fn create_product(
        &self,
        Json(body): Json<CreateProductDto>,
    ) -> Result<Json<Product>, HttpError> {
        if body.title.trim().len() < 2 {
            return Err(HttpError::bad_request(
                "title must be at least 2 characters",
            ));
        }
        if !(body.price >= 0.0) {
            return Err(HttpError::bad_request("price must be >= 0"));
        }
        if body.stock < 0 {
            return Err(HttpError::bad_request("stock must be >= 0"));
        }
        let cmd = CreateProductCommand::from_dto(body);
        let product_id = cmd.id.clone();
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        match self
            .queries
            .dispatch(GetProductQuery {
                product_id: product_id.clone(),
            })
            .await
        {
            Ok(Some(p)) => Ok(Json(p)),
            Ok(None) => Err(HttpError::not_found(format!(
                "product {product_id} not found after insert"
            ))),
            Err(e) => Err(HttpError::internal(format!("query failed: {e}"))),
        }
    }

    #[post("/commands/{id}/stock")]
    #[use_guards(AuthGuard)]
    pub async fn update_stock(
        &self,
        Path(id): Path<String>,
        Json(body): Json<UpdateStockDto>,
    ) -> Result<Json<Product>, HttpError> {
        let cmd = UpdateProductStockCommand {
            product_id: id.clone(),
            delta: body.delta,
        };
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        match self
            .queries
            .dispatch(GetProductQuery {
                product_id: id.clone(),
            })
            .await
        {
            Ok(Some(p)) => Ok(Json(p)),
            Ok(None) => Err(HttpError::not_found(format!("product {id} not found"))),
            Err(e) => Err(HttpError::internal(format!("query failed: {e}"))),
        }
    }

    #[get("/queries/{id}")]
    pub async fn get_product(&self, Path(id): Path<String>) -> Json<Option<Product>> {
        Json(
            self.queries
                .dispatch(GetProductQuery { product_id: id })
                .await
                .unwrap_or(None),
        )
    }

    #[get("/queries")]
    pub async fn list_products(&self, Query(q): Query<ListParams>) -> Json<ProductList> {
        let q = ListProductsQuery {
            limit: q.limit.unwrap_or(20).clamp(1, 100),
            offset: q.offset.unwrap_or(0),
        };
        Json(self.queries.dispatch(q).await.unwrap_or(ProductList {
            total: 0,
            limit: 20,
            offset: 0,
            items: vec![],
        }))
    }
}
