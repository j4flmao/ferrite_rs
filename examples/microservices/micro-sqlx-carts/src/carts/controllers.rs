use crate::carts::dto::{
    AddToCartCommand, AddToCartDto, Cart, ClearCartCommand, ConvertCartCommand, ConvertCartDto,
    CreateCartCommand, CreateCartDto, GetCartQuery, GetSessionCartQuery, GetUserCartQuery,
    ListCartsQuery, RemoveFromCartCommand, UpdateCartItemCommand, UpdateCartItemDto,
};
use crate::carts::repo::CartsRepo;
use crate::carts::service::CartsService;
use ferrite_auth_jwt::AuthGuard;
use ferrite_cqrs::{CommandBus, EventBus, QueryBus};
use ferrite_framework::{controller, impl_controller, inject, HttpError, Json, Path, Query};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[controller("/carts")]
pub struct CartsController {
    commands: CommandBus,
    queries: QueryBus,
    events: EventBus,
    repo: CartsRepo,
    service: CartsService,
}

#[impl_controller]
impl CartsController {
    #[inject]
    pub fn new(
        commands: CommandBus,
        queries: QueryBus,
        events: EventBus,
        repo: CartsRepo,
        service: CartsService,
    ) -> Self {
        Self {
            commands,
            queries,
            events,
            repo,
            service,
        }
    }

    #[post("/commands/create")]
    pub async fn create_cart(
        &self,
        Json(body): Json<CreateCartDto>,
    ) -> Result<Json<Cart>, HttpError> {
        let cmd = CreateCartCommand::from_dto(body);
        let cart_id = cmd.id.clone();
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        match self
            .queries
            .dispatch(GetCartQuery {
                cart_id: cart_id.clone(),
            })
            .await
        {
            Ok(Some(c)) => Ok(Json(c)),
            Ok(None) => Err(HttpError::not_found(format!(
                "cart {cart_id} not found after create"
            ))),
            Err(e) => Err(HttpError::internal(format!("query failed: {e}"))),
        }
    }

    #[get("/queries/{id}")]
    pub async fn get_cart(&self, Path(id): Path<String>) -> Json<Option<Cart>> {
        Json(
            self.queries
                .dispatch(GetCartQuery { cart_id: id })
                .await
                .unwrap_or(None),
        )
    }

    #[get("/queries/user/{user_id}")]
    #[use_guards(AuthGuard)]
    pub async fn get_user_cart(&self, Path(user_id): Path<i64>) -> Json<Option<Cart>> {
        Json(
            self.queries
                .dispatch(GetUserCartQuery { user_id })
                .await
                .unwrap_or(None),
        )
    }

    #[get("/queries/session/{session_id}")]
    pub async fn get_session_cart(&self, Path(session_id): Path<String>) -> Json<Option<Cart>> {
        Json(
            self.queries
                .dispatch(GetSessionCartQuery { session_id })
                .await
                .unwrap_or(None),
        )
    }

    #[get("/queries")]
    pub async fn list_carts(
        &self,
        Query(q): Query<ListParams>,
    ) -> Json<crate::carts::dto::CartList> {
        let q = ListCartsQuery {
            limit: q.limit.unwrap_or(20).clamp(1, 100),
            offset: q.offset.unwrap_or(0),
        };
        Json(
            self.queries
                .dispatch(q)
                .await
                .unwrap_or(crate::carts::dto::CartList {
                    total: 0,
                    limit: 20,
                    offset: 0,
                    items: vec![],
                }),
        )
    }

    #[post("/commands/{id}/clear")]
    pub async fn clear_cart(&self, Path(id): Path<String>) -> Result<Json<Cart>, HttpError> {
        let cmd = ClearCartCommand {
            cart_id: id.clone(),
        };
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        match self
            .queries
            .dispatch(GetCartQuery {
                cart_id: id.clone(),
            })
            .await
        {
            Ok(Some(c)) => Ok(Json(c)),
            Ok(None) => Err(HttpError::not_found(format!("cart {id} not found"))),
            Err(e) => Err(HttpError::internal(format!("query failed: {e}"))),
        }
    }

    #[post("/commands/{id}/convert")]
    #[use_guards(AuthGuard)]
    pub async fn convert_cart(
        &self,
        Path(id): Path<String>,
        Json(body): Json<ConvertCartDto>,
    ) -> Result<Json<Cart>, HttpError> {
        if body.user_id <= 0 {
            return Err(HttpError::bad_request("user_id must be > 0"));
        }
        let cmd = ConvertCartCommand {
            cart_id: id.clone(),
            user_id: body.user_id,
        };
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        match self
            .queries
            .dispatch(GetCartQuery {
                cart_id: id.clone(),
            })
            .await
        {
            Ok(Some(c)) => Ok(Json(c)),
            Ok(None) => Err(HttpError::not_found(format!("cart {id} not found"))),
            Err(e) => Err(HttpError::internal(format!("query failed: {e}"))),
        }
    }
}

#[controller("/carts/{cart_id}/items")]
pub struct ItemsController {
    commands: CommandBus,
    queries: QueryBus,
    events: EventBus,
    repo: CartsRepo,
    service: CartsService,
}

#[impl_controller]
impl ItemsController {
    #[inject]
    pub fn new(
        commands: CommandBus,
        queries: QueryBus,
        events: EventBus,
        repo: CartsRepo,
        service: CartsService,
    ) -> Self {
        Self {
            commands,
            queries,
            events,
            repo,
            service,
        }
    }

    #[post("/commands/add")]
    pub async fn add_item(
        &self,
        Path(cart_id): Path<String>,
        Json(body): Json<AddToCartDto>,
    ) -> Result<Json<Cart>, HttpError> {
        if body.quantity < 1 {
            return Err(HttpError::bad_request("quantity must be >= 1"));
        }
        if body.unit_price < 0.0 {
            return Err(HttpError::bad_request("unit_price must be >= 0"));
        }
        if body.product_id.trim().is_empty() {
            return Err(HttpError::bad_request("product_id must not be empty"));
        }

        let item_id = CartsService::new_item_id();
        let cmd = AddToCartCommand {
            cart_id: cart_id.clone(),
            item_id,
            product_id: body.product_id,
            product_title: body.product_title,
            unit_price: body.unit_price,
            quantity: body.quantity,
        };
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        match self
            .queries
            .dispatch(GetCartQuery {
                cart_id: cart_id.clone(),
            })
            .await
        {
            Ok(Some(c)) => Ok(Json(c)),
            Ok(None) => Err(HttpError::not_found(format!("cart {cart_id} not found"))),
            Err(e) => Err(HttpError::internal(format!("query failed: {e}"))),
        }
    }

    #[put("/commands/{item_id}/update")]
    pub async fn update_item(
        &self,
        Path((cart_id, item_id)): Path<(String, String)>,
        Json(body): Json<UpdateCartItemDto>,
    ) -> Result<Json<Cart>, HttpError> {
        if body.quantity < 1 {
            return Err(HttpError::bad_request("quantity must be >= 1"));
        }
        let cmd = UpdateCartItemCommand {
            cart_id: cart_id.clone(),
            item_id: item_id.clone(),
            quantity: body.quantity,
        };
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        match self
            .queries
            .dispatch(GetCartQuery {
                cart_id: cart_id.clone(),
            })
            .await
        {
            Ok(Some(c)) => Ok(Json(c)),
            Ok(None) => Err(HttpError::not_found(format!("cart {cart_id} not found"))),
            Err(e) => Err(HttpError::internal(format!("query failed: {e}"))),
        }
    }

    #[delete("/commands/{item_id}/remove")]
    pub async fn remove_item(
        &self,
        Path((cart_id, item_id)): Path<(String, String)>,
    ) -> Result<Json<Cart>, HttpError> {
        let cmd = RemoveFromCartCommand {
            cart_id: cart_id.clone(),
            item_id: item_id.clone(),
        };
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        match self
            .queries
            .dispatch(GetCartQuery {
                cart_id: cart_id.clone(),
            })
            .await
        {
            Ok(Some(c)) => Ok(Json(c)),
            Ok(None) => Err(HttpError::not_found(format!("cart {cart_id} not found"))),
            Err(e) => Err(HttpError::internal(format!("query failed: {e}"))),
        }
    }
}
