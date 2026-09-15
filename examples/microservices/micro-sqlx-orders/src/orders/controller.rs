use crate::orders::dto::{
    CancelOrderDto, CreateOrderDto, GetOrderQuery, ListOrdersQuery, OrderList, OrderWithItems,
    UpdateOrderStatusDto,
};
use crate::orders::service::OrdersService;
use ferrite_auth_jwt::{AuthGuard, CurrentUser};
use ferrite_cqrs::{CommandBus, EventBus, QueryBus};
use ferrite_framework::{controller, impl_controller, inject, HttpError, Json, Path, Query};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub user_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwtClaims {
    pub sub: i64,
    pub email: Option<String>,
    pub admin: Option<bool>,
}

impl JwtClaims {
    pub fn is_admin(&self) -> bool {
        self.admin.unwrap_or(false)
    }
}

fn claims_to_claims(claims: &ferrite_auth_jwt::JwtClaims) -> JwtClaims {
    JwtClaims {
        sub: claims.sub,
        email: Some(claims.email.clone()),
        admin: None,
    }
}

#[controller("/orders")]
pub struct OrdersController {
    commands: CommandBus,
    queries: QueryBus,
    events: EventBus,
    service: OrdersService,
}

#[impl_controller]
impl OrdersController {
    #[inject]
    pub fn new(
        commands: CommandBus,
        queries: QueryBus,
        events: EventBus,
        service: OrdersService,
    ) -> Self {
        Self {
            commands,
            queries,
            events,
            service,
        }
    }

    #[post("/commands/create")]
    #[use_guards(AuthGuard)]
    pub async fn create_order(
        &self,
        CurrentUser(claims): CurrentUser,
        Json(body): Json<CreateOrderDto>,
    ) -> Result<Json<OrderWithItems>, HttpError> {
        let c = claims_to_claims(&claims);
        Ok(Json(self.service.create_order(body, c.sub).await?))
    }

    #[post("/commands/{id}/status")]
    #[use_guards(AuthGuard)]
    pub async fn update_status(
        &self,
        CurrentUser(claims): CurrentUser,
        Path(id): Path<String>,
        Json(body): Json<UpdateOrderStatusDto>,
    ) -> Result<Json<OrderWithItems>, HttpError> {
        let c = claims_to_claims(&claims);
        if !c.is_admin() {
            return Err(HttpError::forbidden(
                "admin role required to update order status",
            ));
        }
        Ok(Json(self.service.update_status(&id, &body.status).await?))
    }

    #[post("/commands/{id}/cancel")]
    #[use_guards(AuthGuard)]
    pub async fn cancel_order(
        &self,
        CurrentUser(claims): CurrentUser,
        Path(id): Path<String>,
        Json(body): Json<CancelOrderDto>,
    ) -> Result<Json<OrderWithItems>, HttpError> {
        let c = claims_to_claims(&claims);
        Ok(Json(
            self.service.cancel_order(&id, c.sub, body.reason).await?,
        ))
    }

    #[get("/queries/{id}")]
    #[use_guards(AuthGuard)]
    pub async fn get_order(
        &self,
        CurrentUser(claims): CurrentUser,
        Path(id): Path<String>,
    ) -> Result<Json<Option<OrderWithItems>>, HttpError> {
        let c = claims_to_claims(&claims);
        let q = GetOrderQuery {
            order_id: id,
            user_id: Some(c.sub),
            is_admin: c.is_admin(),
        };
        match self.queries.dispatch(q).await {
            Ok(result) => Ok(Json(result)),
            Err(e) => Err(HttpError::internal(format!("query failed: {e}"))),
        }
    }

    #[get("/queries")]
    #[use_guards(AuthGuard)]
    pub async fn list_orders(
        &self,
        CurrentUser(claims): CurrentUser,
        Query(q): Query<ListParams>,
    ) -> Result<Json<OrderList>, HttpError> {
        let c = claims_to_claims(&claims);
        let limit = q.limit.unwrap_or(20).clamp(1, 100);
        let offset = q.offset.unwrap_or(0);
        let _list_q = ListOrdersQuery {
            user_id: Some(c.sub),
            is_admin: c.is_admin(),
            limit,
            offset,
        };
        let (total, items) = self
            .service
            .list_orders(Some(c.sub), c.is_admin(), q.user_id, limit, offset)
            .await?;
        Ok(Json(OrderList {
            total,
            limit,
            offset,
            items,
        }))
    }
}
