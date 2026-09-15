use crate::orders::dto::{
    is_valid_status, CancelOrderCommand, CreateOrderCommand, CreateOrderDto, Order, OrderItem,
    OrderItemDto, OrderWithItems, UpdateOrderStatusCommand,
};
use crate::orders::repo::{OrderItemsRepo, OrdersRepo};
use ferrite_cqrs::CommandBus;
use ferrite_framework::{inject, HttpError};
use ferrite_macros::injectable as injectable_attr;
use uuid::Uuid;

#[injectable_attr]
pub struct OrdersService {
    orders_repo: OrdersRepo,
    items_repo: OrderItemsRepo,
    commands: CommandBus,
}

impl OrdersService {
    #[inject]
    pub fn new(orders_repo: OrdersRepo, items_repo: OrderItemsRepo, commands: CommandBus) -> Self {
        Self {
            orders_repo,
            items_repo,
            commands,
        }
    }

    pub fn validate_create_dto(&self, dto: &CreateOrderDto) -> Result<(), HttpError> {
        if dto.items.is_empty() {
            return Err(HttpError::bad_request(
                "order must contain at least one item",
            ));
        }
        for item in &dto.items {
            if item.quantity < 1 {
                return Err(HttpError::bad_request("each item quantity must be >= 1"));
            }
            if item.product_id.trim().is_empty() {
                return Err(HttpError::bad_request(
                    "product_id is required for each item",
                ));
            }
        }
        Ok(())
    }

    pub fn mock_item_prices(&self, items: &[OrderItemDto]) -> Vec<(String, String, f64)> {
        items
            .iter()
            .map(|i| {
                let title = format!("Product {}", &i.product_id[..i.product_id.len().min(8)]);
                let price = 24.99f64;
                (i.product_id.clone(), title, price)
            })
            .collect()
    }

    pub fn build_items(
        &self,
        order_id: &str,
        items: &[OrderItemDto],
        prices: &[(String, String, f64)],
    ) -> Vec<OrderItem> {
        items
            .iter()
            .zip(prices.iter())
            .map(|(item, (_, title, price))| OrderItem {
                id: format!("itm_{}", Uuid::new_v4().simple()),
                order_id: order_id.to_string(),
                product_id: item.product_id.clone(),
                product_title: title.clone(),
                unit_price: *price,
                quantity: item.quantity,
                subtotal: price * item.quantity as f64,
            })
            .collect()
    }

    pub fn calculate_total(&self, items: &[OrderItem]) -> f64 {
        items.iter().map(|i| i.subtotal).sum()
    }

    pub async fn create_order(
        &self,
        dto: CreateOrderDto,
        user_id: i64,
    ) -> Result<OrderWithItems, HttpError> {
        self.validate_create_dto(&dto)?;
        let prices = self.mock_item_prices(&dto.items);
        let mut cmd = CreateOrderCommand::from_dto(dto.clone(), user_id, prices.clone());
        let total = self.calculate_total(&self.build_items(&cmd.id, &cmd.items, &prices));
        cmd.total_amount = total;
        let order_id = cmd.id.clone();
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        self.get_order_internal(&order_id, Some(user_id), true)
            .await?
            .ok_or_else(|| HttpError::not_found(format!("order {order_id} not found after insert")))
    }

    pub async fn update_status(
        &self,
        order_id: &str,
        status: &str,
    ) -> Result<OrderWithItems, HttpError> {
        if !is_valid_status(status) {
            return Err(HttpError::bad_request(format!(
                "invalid status: {status}. valid: pending, paid, shipped, delivered, cancelled"
            )));
        }
        let cmd = UpdateOrderStatusCommand {
            order_id: order_id.to_string(),
            status: status.to_string(),
        };
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        self.get_order_internal(order_id, None, true)
            .await?
            .ok_or_else(|| HttpError::not_found(format!("order {order_id} not found")))
    }

    pub async fn cancel_order(
        &self,
        order_id: &str,
        user_id: i64,
        reason: Option<String>,
    ) -> Result<OrderWithItems, HttpError> {
        let order = self.orders_repo.get(order_id).await;
        let order =
            order.ok_or_else(|| HttpError::not_found(format!("order {order_id} not found")))?;
        if order.user_id != user_id {
            return Err(HttpError::forbidden("you can only cancel your own orders"));
        }
        if order.status != "pending" {
            return Err(HttpError::bad_request(
                "only pending orders can be cancelled",
            ));
        }
        let cmd = CancelOrderCommand {
            order_id: order_id.to_string(),
            user_id,
            reason,
        };
        self.commands
            .dispatch(cmd)
            .await
            .map_err(|e| HttpError::bad_request(format!("command dispatch failed: {e}")))?;
        self.get_order_internal(order_id, Some(user_id), false)
            .await?
            .ok_or_else(|| HttpError::not_found(format!("order {order_id} not found")))
    }

    pub async fn get_order_internal(
        &self,
        order_id: &str,
        user_id: Option<i64>,
        is_admin: bool,
    ) -> Result<Option<OrderWithItems>, HttpError> {
        let order = match self.orders_repo.get(order_id).await {
            Some(o) => o,
            None => return Ok(None),
        };
        if !is_admin {
            if let Some(uid) = user_id {
                if order.user_id != uid {
                    return Err(HttpError::forbidden("you do not have access to this order"));
                }
            }
        }
        let items = self.items_repo.list_by_order(order_id).await;
        Ok(Some(OrderWithItems { order, items }))
    }

    pub async fn get_order(
        &self,
        order_id: &str,
        user_id: Option<i64>,
        is_admin: bool,
    ) -> Result<Option<OrderWithItems>, HttpError> {
        self.get_order_internal(order_id, user_id, is_admin).await
    }

    pub async fn list_orders(
        &self,
        user_id: Option<i64>,
        is_admin: bool,
        filter_user_id: Option<i64>,
        limit: usize,
        offset: usize,
    ) -> Result<(usize, Vec<OrderWithItems>), HttpError> {
        let (total, orders) = if is_admin {
            if let Some(fuid) = filter_user_id {
                self.orders_repo.list_by_user(fuid, limit, offset).await
            } else {
                self.orders_repo.list_all(limit, offset).await
            }
        } else {
            let uid = user_id.ok_or_else(|| HttpError::unauthorized("authentication required"))?;
            self.orders_repo.list_by_user(uid, limit, offset).await
        };
        let mut with_items = Vec::with_capacity(orders.len());
        for o in orders {
            let items = self.items_repo.list_by_order(&o.id).await;
            with_items.push(OrderWithItems { order: o, items });
        }
        Ok((total, with_items))
    }
}

#[allow(dead_code)]
pub fn _force_use_order(o: Order) {
    let _ = o;
}
