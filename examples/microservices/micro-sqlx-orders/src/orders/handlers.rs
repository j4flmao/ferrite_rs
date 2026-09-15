use crate::orders::dto::{
    CancelOrderCommand, CreateOrderCommand, GetOrderQuery, ListOrdersQuery, Order,
    OrderCancelledEvent, OrderCreatedEvent, OrderItem, OrderList, OrderStatusChangedEvent,
    OrderWithItems, UpdateOrderStatusCommand,
};
use crate::orders::repo::{OrderItemsRepo, OrdersRepo};
use async_trait::async_trait;
use ferrite_cache_redis::CacheService;
use ferrite_cqrs::{
    submit_command_handler, submit_event_handler, submit_query_handler, CommandBus, CommandHandler,
    EventBus, EventHandler, QueryBus, QueryHandler,
};
use ferrite_kafka::{KafkaError, KafkaHandler, KafkaMessage, KafkaServer};
use ferrite_macros::{inject, injectable};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

ferrite_cqrs::__private_shim_command!(CreateOrderHandler, CreateOrderCommand);
ferrite_cqrs::__private_shim_command!(UpdateOrderStatusHandler, UpdateOrderStatusCommand);
ferrite_cqrs::__private_shim_command!(CancelOrderHandler, CancelOrderCommand);
ferrite_cqrs::__private_shim_query!(GetOrderHandler, GetOrderQuery);
ferrite_cqrs::__private_shim_query!(ListOrdersHandler, ListOrdersQuery);
ferrite_cqrs::__private_shim_event!(OrderCreatedEventHandler, OrderCreatedEvent);
ferrite_cqrs::__private_shim_event!(OrderStatusChangedEventHandler, OrderStatusChangedEvent);
ferrite_cqrs::__private_shim_event!(AuditOrderEventsHandler, OrderCreatedEvent);

// ========== Command Handlers ==========
#[injectable]
pub struct CreateOrderHandler {
    orders_repo: OrdersRepo,
    items_repo: OrderItemsRepo,
    events: EventBus,
}

impl CreateOrderHandler {
    #[inject]
    pub fn new(orders_repo: OrdersRepo, items_repo: OrderItemsRepo, events: EventBus) -> Self {
        Self {
            orders_repo,
            items_repo,
            events,
        }
    }
}

#[async_trait]
impl CommandHandler<CreateOrderCommand> for CreateOrderHandler {
    async fn handle(&self, cmd: CreateOrderCommand) {
        let now = chrono::Utc::now().to_rfc3339();
        let order = Order {
            id: cmd.id.clone(),
            user_id: cmd.user_id,
            status: "pending".into(),
            total_amount: cmd.total_amount,
            currency: cmd.currency,
            shipping_address: cmd.shipping_address.clone(),
            notes: cmd.notes.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        let mut order_items: Vec<OrderItem> = Vec::new();
        for item in &cmd.items {
            let unit_price = 24.99f64;
            let product_title = format!(
                "Product {}",
                &item.product_id[..item.product_id.len().min(8)]
            );
            let quantity = item.quantity;
            let subtotal = unit_price * quantity as f64;
            let order_item = OrderItem {
                id: format!("itm_{}", Uuid::new_v4().simple()),
                order_id: cmd.id.clone(),
                product_id: item.product_id.clone(),
                product_title,
                unit_price,
                quantity,
                subtotal,
            };
            order_items.push(order_item);
        }
        let total = order_items.iter().map(|i| i.subtotal).sum::<f64>();
        let mut order = order;
        order.total_amount = total;
        let items_clone = order_items.clone();
        let event = OrderCreatedEvent {
            order: order.clone(),
            items: items_clone,
            at: chrono::Utc::now().to_rfc3339(),
        };
        self.orders_repo.insert(order).await;
        for item in order_items {
            self.items_repo.insert(item).await;
        }
        let _ = self.events.publish(event).await;
    }
}

submit_command_handler!(CreateOrderHandler, CreateOrderCommand);

#[injectable]
pub struct UpdateOrderStatusHandler {
    repo: OrdersRepo,
    events: EventBus,
    publisher: KafkaOrderPublisher,
}

impl UpdateOrderStatusHandler {
    #[inject]
    pub fn new(repo: OrdersRepo, events: EventBus, publisher: KafkaOrderPublisher) -> Self {
        Self {
            repo,
            events,
            publisher,
        }
    }
}

#[async_trait]
impl CommandHandler<UpdateOrderStatusCommand> for UpdateOrderStatusHandler {
    async fn handle(&self, cmd: UpdateOrderStatusCommand) {
        if let Some((old, new, _order)) = self.repo.update_status(&cmd.order_id, &cmd.status).await
        {
            let ev = OrderStatusChangedEvent {
                order_id: cmd.order_id,
                old_status: old,
                new_status: new,
                at: chrono::Utc::now().to_rfc3339(),
            };
            let ev_clone = ev.clone();
            let _ = self.events.publish(ev).await;
            let _ = self.publisher.publish_status_changed(&ev_clone).await;
        }
    }
}

submit_command_handler!(UpdateOrderStatusHandler, UpdateOrderStatusCommand);

#[injectable]
pub struct CancelOrderHandler {
    repo: OrdersRepo,
    events: EventBus,
}

impl CancelOrderHandler {
    #[inject]
    pub fn new(repo: OrdersRepo, events: EventBus) -> Self {
        Self { repo, events }
    }
}

#[async_trait]
impl CommandHandler<CancelOrderCommand> for CancelOrderHandler {
    async fn handle(&self, cmd: CancelOrderCommand) {
        if let Some(_order) = self.repo.cancel(&cmd.order_id).await {
            let ev = OrderCancelledEvent {
                order_id: cmd.order_id,
                user_id: cmd.user_id,
                reason: cmd.reason,
                at: chrono::Utc::now().to_rfc3339(),
            };
            let _ = self.events.publish(ev).await;
        }
    }
}

submit_command_handler!(CancelOrderHandler, CancelOrderCommand);

// ========== Query Handlers ==========
#[injectable]
pub struct GetOrderHandler {
    orders_repo: OrdersRepo,
    items_repo: OrderItemsRepo,
}

impl GetOrderHandler {
    #[inject]
    pub fn new(orders_repo: OrdersRepo, items_repo: OrderItemsRepo) -> Self {
        Self {
            orders_repo,
            items_repo,
        }
    }
}

#[async_trait]
impl QueryHandler<GetOrderQuery> for GetOrderHandler {
    async fn handle(&self, q: GetOrderQuery) -> Option<OrderWithItems> {
        let order = self.orders_repo.get(&q.order_id).await?;
        if !q.is_admin {
            if let Some(uid) = q.user_id {
                if order.user_id != uid {
                    return None;
                }
            }
        }
        let items = self.items_repo.list_by_order(&q.order_id).await;
        Some(OrderWithItems { order, items })
    }
}

submit_query_handler!(GetOrderHandler, GetOrderQuery);

#[injectable]
pub struct ListOrdersHandler {
    orders_repo: OrdersRepo,
    items_repo: OrderItemsRepo,
}

impl ListOrdersHandler {
    #[inject]
    pub fn new(orders_repo: OrdersRepo, items_repo: OrderItemsRepo) -> Self {
        Self {
            orders_repo,
            items_repo,
        }
    }
}

#[async_trait]
impl QueryHandler<ListOrdersQuery> for ListOrdersHandler {
    async fn handle(&self, q: ListOrdersQuery) -> OrderList {
        let (total, orders) = if q.is_admin {
            if let Some(uid) = q.user_id {
                self.orders_repo.list_by_user(uid, q.limit, q.offset).await
            } else {
                self.orders_repo.list_all(q.limit, q.offset).await
            }
        } else if let Some(uid) = q.user_id {
            self.orders_repo.list_by_user(uid, q.limit, q.offset).await
        } else {
            (0, vec![])
        };
        let mut items = Vec::with_capacity(orders.len());
        for o in orders {
            let order_items = self.items_repo.list_by_order(&o.id).await;
            items.push(OrderWithItems {
                order: o,
                items: order_items,
            });
        }
        OrderList {
            total,
            limit: q.limit,
            offset: q.offset,
            items,
        }
    }
}

submit_query_handler!(ListOrdersHandler, ListOrdersQuery);

// ========== Event Handlers (fan-out) ==========
#[injectable]
pub struct OrderCreatedEventHandler {
    publisher: KafkaOrderPublisher,
    cache: CacheService,
}

impl OrderCreatedEventHandler {
    #[inject]
    pub fn new(publisher: KafkaOrderPublisher, cache: CacheService) -> Self {
        Self { publisher, cache }
    }
}

#[async_trait]
impl EventHandler<OrderCreatedEvent> for OrderCreatedEventHandler {
    async fn handle(&self, ev: OrderCreatedEvent) {
        let reserve_key = format!("reserve:{}", ev.order.id);
        let _ = self
            .cache
            .set_with_ttl(&reserve_key, &ev.order.id, 900)
            .await;
        let _ = self.publisher.publish_created(&ev).await;
    }
}

submit_event_handler!(OrderCreatedEventHandler, OrderCreatedEvent);

#[injectable]
pub struct OrderStatusChangedEventHandler {
    publisher: KafkaOrderPublisher,
}

impl OrderStatusChangedEventHandler {
    #[inject]
    pub fn new(publisher: KafkaOrderPublisher) -> Self {
        Self { publisher }
    }
}

#[async_trait]
impl EventHandler<OrderStatusChangedEvent> for OrderStatusChangedEventHandler {
    async fn handle(&self, ev: OrderStatusChangedEvent) {
        let _ = self.publisher.publish_status_changed(&ev).await;
    }
}

submit_event_handler!(OrderStatusChangedEventHandler, OrderStatusChangedEvent);

#[injectable]
pub struct AuditOrderEventsHandler {
    log: Mutex<Vec<String>>,
}

impl AuditOrderEventsHandler {
    #[inject]
    pub fn new() -> Self {
        Self {
            log: Mutex::new(Vec::new()).into(),
        }
    }
}

#[async_trait]
impl EventHandler<OrderCreatedEvent> for AuditOrderEventsHandler {
    async fn handle(&self, ev: OrderCreatedEvent) {
        let mut g = self.log.lock().await;
        g.push(format!(
            "[{}] ORDER_CREATED id={} user={} total={} items={}",
            ev.at,
            ev.order.id,
            ev.order.user_id,
            ev.order.total_amount,
            ev.items.len()
        ));
        let cap = g.len();
        if cap > 1000 {
            let drop_n = cap - 1000;
            g.drain(0..drop_n);
        }
    }
}

submit_event_handler!(AuditOrderEventsHandler, OrderCreatedEvent);

// ========== Kafka publisher + pattern consumer ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderEventEnvelope {
    pub kind: String,
    pub id: String,
    pub payload: serde_json::Value,
}

#[injectable]
pub struct KafkaOrderPublisher {
    kafka: KafkaServer,
}

impl KafkaOrderPublisher {
    #[inject]
    pub fn new(kafka: KafkaServer) -> Self {
        Self { kafka }
    }

    pub async fn publish_created(&self, ev: &OrderCreatedEvent) -> Result<(), String> {
        let env = OrderEventEnvelope {
            kind: "order.created".into(),
            id: ev.order.id.clone(),
            payload: serde_json::to_value(ev).map_err(|e| e.to_string())?,
        };
        self.kafka
            .produce("orders.events", Some(ev.order.id.as_str()), &env)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn publish_status_changed(&self, ev: &OrderStatusChangedEvent) -> Result<(), String> {
        let env = OrderEventEnvelope {
            kind: "order.status_changed".into(),
            id: ev.order_id.clone(),
            payload: serde_json::to_value(ev).map_err(|e| e.to_string())?,
        };
        self.kafka
            .produce("orders.events", Some(ev.order_id.as_str()), &env)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn publish_checkout_completed(
        &self,
        order_id: &str,
        payment_url: &str,
        expires_at: &str,
    ) -> Result<(), String> {
        let payload = serde_json::json!({
            "order_id": order_id,
            "payment_url": payment_url,
            "expires_at": expires_at,
        });
        let env = OrderEventEnvelope {
            kind: "checkout.completed".into(),
            id: order_id.to_string(),
            payload,
        };
        self.kafka
            .produce("orders.events", Some(order_id), &env)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[injectable]
pub struct OrdersEventsKafkaHandler {
    processed: Mutex<Vec<String>>,
}

impl OrdersEventsKafkaHandler {
    #[inject]
    pub fn new() -> Self {
        Self {
            processed: Mutex::new(Vec::new()).into(),
        }
    }
}

#[async_trait]
impl KafkaHandler<OrderEventEnvelope> for OrdersEventsKafkaHandler {
    const PATTERN: &'static str = "orders.events";
    async fn handle(&self, msg: KafkaMessage<OrderEventEnvelope>) -> Result<(), KafkaError> {
        let mut g = self.processed.lock().await;
        g.push(format!(
            "topic={} off={} kind={} id={}",
            msg.topic, msg.offset, msg.payload.kind, msg.payload.id
        ));
        let cap = g.len();
        if cap > 5000 {
            let drop_n = cap - 5000;
            g.drain(0..drop_n);
        }
        drop(g);
        tokio::time::sleep(Duration::from_micros(500)).await;
        Ok(())
    }
}

ferrite_kafka::submit_kafka_handler!(OrdersEventsKafkaHandler, OrderEventEnvelope);

#[allow(dead_code)]
pub fn __force_link_cqrs_buses(_: &CommandBus, _: &QueryBus, _: &EventBus) {}
