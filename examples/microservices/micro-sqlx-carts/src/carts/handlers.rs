use crate::carts::dto::{
    AddToCartCommand, Cart, CartCreatedEvent, CartItemAddedEvent, CartItemRemovedEvent,
    CartItemUpdatedEvent, CartList, CartUpdatedEvent, ClearCartCommand, ConvertCartCommand,
    CreateCartCommand, GetCartQuery, GetSessionCartQuery, GetUserCartQuery, ListCartsQuery,
    RemoveFromCartCommand, UpdateCartItemCommand,
};
use crate::carts::repo::CartsRepo;
use crate::carts::service::CartsService;
use async_trait::async_trait;
use ferrite_cqrs::{
    submit_command_handler, submit_event_handler, submit_query_handler, CommandBus, CommandHandler,
    EventBus, EventHandler, QueryBus, QueryHandler,
};
use ferrite_kafka::{KafkaError, KafkaHandler, KafkaMessage, KafkaServer};
use ferrite_macros::{inject, injectable};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::Mutex;

ferrite_cqrs::__private_shim_command!(CreateCartHandler, CreateCartCommand);
ferrite_cqrs::__private_shim_command!(AddToCartHandler, AddToCartCommand);
ferrite_cqrs::__private_shim_command!(UpdateCartItemHandler, UpdateCartItemCommand);
ferrite_cqrs::__private_shim_command!(RemoveFromCartHandler, RemoveFromCartCommand);
ferrite_cqrs::__private_shim_command!(ClearCartHandler, ClearCartCommand);
ferrite_cqrs::__private_shim_command!(ConvertCartHandler, ConvertCartCommand);
ferrite_cqrs::__private_shim_query!(GetCartHandler, GetCartQuery);
ferrite_cqrs::__private_shim_query!(GetUserCartHandler, GetUserCartQuery);
ferrite_cqrs::__private_shim_query!(GetSessionCartHandler, GetSessionCartQuery);
ferrite_cqrs::__private_shim_query!(ListCartsHandler, ListCartsQuery);
ferrite_cqrs::__private_shim_event!(CartCreatedEventHandler, CartCreatedEvent);
ferrite_cqrs::__private_shim_event!(CartUpdatedEventHandler, CartUpdatedEvent);
ferrite_cqrs::__private_shim_event!(AuditCartEventsHandler, CartCreatedEvent);
ferrite_cqrs::__private_shim_event!(AuditCartEventsHandler, CartUpdatedEvent);

fn build_cart_from_command(cmd: CreateCartCommand) -> Cart {
    let now = chrono::Utc::now();
    let expires_at = if cmd.user_id == -1 {
        Some((now + chrono::Duration::seconds(86400)).to_rfc3339())
    } else {
        None
    };
    Cart {
        id: cmd.id,
        user_id: cmd.user_id,
        session_id: cmd.session_id,
        status: "active".into(),
        total_amount: 0.0,
        item_count: 0,
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
        expires_at,
        items: vec![],
    }
}

// ========== Command Handlers ==========
#[injectable]
pub struct CreateCartHandler {
    repo: CartsRepo,
    events: EventBus,
}

impl CreateCartHandler {
    #[inject]
    pub fn new(repo: CartsRepo, events: EventBus) -> Self {
        Self { repo, events }
    }
}

#[async_trait]
impl CommandHandler<CreateCartCommand> for CreateCartHandler {
    async fn handle(&self, cmd: CreateCartCommand) {
        let cart = build_cart_from_command(cmd);
        let event = CartCreatedEvent {
            cart: cart.clone(),
            at: chrono::Utc::now().to_rfc3339(),
        };
        self.repo.create(cart).await;
        let _ = self.events.publish(event).await;
    }
}

submit_command_handler!(CreateCartHandler, CreateCartCommand);

#[injectable]
pub struct AddToCartHandler {
    repo: CartsRepo,
    events: EventBus,
    publisher: KafkaCartPublisher,
}

impl AddToCartHandler {
    #[inject]
    pub fn new(repo: CartsRepo, events: EventBus, publisher: KafkaCartPublisher) -> Self {
        Self {
            repo,
            events,
            publisher,
        }
    }
}

#[async_trait]
impl CommandHandler<AddToCartCommand> for AddToCartHandler {
    async fn handle(&self, cmd: AddToCartCommand) {
        let item = CartsService::build_cart_item(
            &cmd.cart_id,
            &cmd.product_id,
            &cmd.product_title,
            cmd.unit_price,
            cmd.quantity,
        );
        if let Some(cart) = self.repo.add_item(&cmd.cart_id, item.clone()).await {
            let ev = CartItemAddedEvent {
                cart_id: cmd.cart_id.clone(),
                item,
                at: chrono::Utc::now().to_rfc3339(),
            };
            let _ = self.events.publish(ev).await;
            let updated = CartUpdatedEvent {
                cart,
                at: chrono::Utc::now().to_rfc3339(),
            };
            let updated_clone = updated.clone();
            let _ = self.events.publish(updated).await;
            let _ = self.publisher.publish_cart_updated(&updated_clone).await;
        }
    }
}

submit_command_handler!(AddToCartHandler, AddToCartCommand);

#[injectable]
pub struct UpdateCartItemHandler {
    repo: CartsRepo,
    events: EventBus,
    publisher: KafkaCartPublisher,
}

impl UpdateCartItemHandler {
    #[inject]
    pub fn new(repo: CartsRepo, events: EventBus, publisher: KafkaCartPublisher) -> Self {
        Self {
            repo,
            events,
            publisher,
        }
    }
}

#[async_trait]
impl CommandHandler<UpdateCartItemCommand> for UpdateCartItemHandler {
    async fn handle(&self, cmd: UpdateCartItemCommand) {
        if let Some((cart, old_qty, new_qty)) = self
            .repo
            .update_item(&cmd.cart_id, &cmd.item_id, cmd.quantity)
            .await
        {
            let ev = CartItemUpdatedEvent {
                cart_id: cmd.cart_id.clone(),
                item_id: cmd.item_id,
                old_quantity: old_qty,
                new_quantity: new_qty,
                at: chrono::Utc::now().to_rfc3339(),
            };
            let _ = self.events.publish(ev).await;
            let updated = CartUpdatedEvent {
                cart,
                at: chrono::Utc::now().to_rfc3339(),
            };
            let updated_clone = updated.clone();
            let _ = self.events.publish(updated).await;
            let _ = self.publisher.publish_cart_updated(&updated_clone).await;
        }
    }
}

submit_command_handler!(UpdateCartItemHandler, UpdateCartItemCommand);

#[injectable]
pub struct RemoveFromCartHandler {
    repo: CartsRepo,
    events: EventBus,
}

impl RemoveFromCartHandler {
    #[inject]
    pub fn new(repo: CartsRepo, events: EventBus) -> Self {
        Self { repo, events }
    }
}

#[async_trait]
impl CommandHandler<RemoveFromCartCommand> for RemoveFromCartHandler {
    async fn handle(&self, cmd: RemoveFromCartCommand) {
        if self
            .repo
            .remove_item(&cmd.cart_id, &cmd.item_id)
            .await
            .is_some()
        {
            let ev = CartItemRemovedEvent {
                cart_id: cmd.cart_id.clone(),
                item_id: cmd.item_id,
                at: chrono::Utc::now().to_rfc3339(),
            };
            let _ = self.events.publish(ev).await;
        }
    }
}

submit_command_handler!(RemoveFromCartHandler, RemoveFromCartCommand);

#[injectable]
pub struct ClearCartHandler {
    repo: CartsRepo,
    events: EventBus,
    publisher: KafkaCartPublisher,
}

impl ClearCartHandler {
    #[inject]
    pub fn new(repo: CartsRepo, events: EventBus, publisher: KafkaCartPublisher) -> Self {
        Self {
            repo,
            events,
            publisher,
        }
    }
}

#[async_trait]
impl CommandHandler<ClearCartCommand> for ClearCartHandler {
    async fn handle(&self, cmd: ClearCartCommand) {
        if let Some(cart) = self.repo.clear(&cmd.cart_id).await {
            let updated = CartUpdatedEvent {
                cart,
                at: chrono::Utc::now().to_rfc3339(),
            };
            let _ = self.events.publish(updated).await;
        }
    }
}

submit_command_handler!(ClearCartHandler, ClearCartCommand);

#[injectable]
pub struct ConvertCartHandler {
    repo: CartsRepo,
    events: EventBus,
}

impl ConvertCartHandler {
    #[inject]
    pub fn new(repo: CartsRepo, events: EventBus) -> Self {
        Self { repo, events }
    }
}

#[async_trait]
impl CommandHandler<ConvertCartCommand> for ConvertCartHandler {
    async fn handle(&self, cmd: ConvertCartCommand) {
        if let Some(cart) = self.repo.convert(&cmd.cart_id, cmd.user_id).await {
            let updated = CartUpdatedEvent {
                cart,
                at: chrono::Utc::now().to_rfc3339(),
            };
            let _ = self.events.publish(updated).await;
        }
    }
}

submit_command_handler!(ConvertCartHandler, ConvertCartCommand);

// ========== Query Handlers ==========
#[injectable]
pub struct GetCartHandler {
    repo: CartsRepo,
}

impl GetCartHandler {
    #[inject]
    pub fn new(repo: CartsRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl QueryHandler<GetCartQuery> for GetCartHandler {
    async fn handle(&self, q: GetCartQuery) -> Option<Cart> {
        self.repo.get(&q.cart_id).await
    }
}

submit_query_handler!(GetCartHandler, GetCartQuery);

#[injectable]
pub struct GetUserCartHandler {
    repo: CartsRepo,
}

impl GetUserCartHandler {
    #[inject]
    pub fn new(repo: CartsRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl QueryHandler<GetUserCartQuery> for GetUserCartHandler {
    async fn handle(&self, q: GetUserCartQuery) -> Option<Cart> {
        self.repo
            .get_by_user_or_session(Some(q.user_id), None)
            .await
    }
}

submit_query_handler!(GetUserCartHandler, GetUserCartQuery);

#[injectable]
pub struct GetSessionCartHandler {
    repo: CartsRepo,
}

impl GetSessionCartHandler {
    #[inject]
    pub fn new(repo: CartsRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl QueryHandler<GetSessionCartQuery> for GetSessionCartHandler {
    async fn handle(&self, q: GetSessionCartQuery) -> Option<Cart> {
        self.repo
            .get_by_user_or_session(None, Some(&q.session_id))
            .await
    }
}

submit_query_handler!(GetSessionCartHandler, GetSessionCartQuery);

#[injectable]
pub struct ListCartsHandler {
    repo: CartsRepo,
}

impl ListCartsHandler {
    #[inject]
    pub fn new(repo: CartsRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl QueryHandler<ListCartsQuery> for ListCartsHandler {
    async fn handle(&self, q: ListCartsQuery) -> CartList {
        let (total, items) = self.repo.list_all(q.limit, q.offset).await;
        CartList {
            total,
            limit: q.limit,
            offset: q.offset,
            items,
        }
    }
}

submit_query_handler!(ListCartsHandler, ListCartsQuery);

// ========== Event Handlers (fan-out) ==========
#[injectable]
pub struct CartCreatedEventHandler {
    publisher: KafkaCartPublisher,
}

impl CartCreatedEventHandler {
    #[inject]
    pub fn new(publisher: KafkaCartPublisher) -> Self {
        Self { publisher }
    }
}

#[async_trait]
impl EventHandler<CartCreatedEvent> for CartCreatedEventHandler {
    async fn handle(&self, ev: CartCreatedEvent) {
        let _ = self.publisher.publish_cart_created(&ev).await;
    }
}

submit_event_handler!(CartCreatedEventHandler, CartCreatedEvent);

#[injectable]
pub struct CartUpdatedEventHandler {
    publisher: KafkaCartPublisher,
}

impl CartUpdatedEventHandler {
    #[inject]
    pub fn new(publisher: KafkaCartPublisher) -> Self {
        Self { publisher }
    }
}

#[async_trait]
impl EventHandler<CartUpdatedEvent> for CartUpdatedEventHandler {
    async fn handle(&self, ev: CartUpdatedEvent) {
        let _ = self.publisher.publish_cart_updated(&ev).await;
    }
}

submit_event_handler!(CartUpdatedEventHandler, CartUpdatedEvent);

#[injectable]
pub struct AuditCartEventsHandler {
    log: Mutex<Vec<String>>,
}

impl AuditCartEventsHandler {
    #[inject]
    pub fn new() -> Self {
        Self {
            log: Mutex::new(Vec::new()).into(),
        }
    }
}

#[async_trait]
impl EventHandler<CartCreatedEvent> for AuditCartEventsHandler {
    async fn handle(&self, ev: CartCreatedEvent) {
        let mut g = self.log.lock().await;
        g.push(format!(
            "[{}] CART_CREATED id={} user={} session={:?}",
            ev.at, ev.cart.id, ev.cart.user_id, ev.cart.session_id
        ));
        let cap = g.len();
        if cap > 1000 {
            let drop_n = cap - 1000;
            g.drain(0..drop_n);
        }
    }
}

submit_event_handler!(AuditCartEventsHandler, CartCreatedEvent);

#[async_trait]
impl EventHandler<CartUpdatedEvent> for AuditCartEventsHandler {
    async fn handle(&self, ev: CartUpdatedEvent) {
        let mut g = self.log.lock().await;
        g.push(format!(
            "[{}] CART_UPDATED id={} total={} items={}",
            ev.at, ev.cart.id, ev.cart.total_amount, ev.cart.item_count
        ));
        let cap = g.len();
        if cap > 1000 {
            let drop_n = cap - 1000;
            g.drain(0..drop_n);
        }
    }
}

submit_event_handler!(AuditCartEventsHandler, CartUpdatedEvent);

// ========== Kafka publisher + pattern consumer ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartEventEnvelope {
    pub kind: String,
    pub id: String,
    pub payload: serde_json::Value,
}

#[injectable]
pub struct KafkaCartPublisher {
    kafka: KafkaServer,
}

impl KafkaCartPublisher {
    #[inject]
    pub fn new(kafka: KafkaServer) -> Self {
        Self { kafka }
    }

    pub async fn publish_cart_created(&self, ev: &CartCreatedEvent) -> Result<(), String> {
        let env = CartEventEnvelope {
            kind: "cart.created".into(),
            id: ev.cart.id.clone(),
            payload: serde_json::to_value(&ev.cart).map_err(|e| e.to_string())?,
        };
        self.kafka
            .produce("carts.events", Some(ev.cart.id.as_str()), &env)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn publish_cart_updated(&self, ev: &CartUpdatedEvent) -> Result<(), String> {
        let env = CartEventEnvelope {
            kind: "cart.updated".into(),
            id: ev.cart.id.clone(),
            payload: serde_json::to_value(&ev.cart).map_err(|e| e.to_string())?,
        };
        self.kafka
            .produce("carts.events", Some(ev.cart.id.as_str()), &env)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[injectable]
pub struct CartsEventsKafkaHandler {
    processed: Mutex<Vec<String>>,
}

impl CartsEventsKafkaHandler {
    #[inject]
    pub fn new() -> Self {
        Self {
            processed: Mutex::new(Vec::new()).into(),
        }
    }
}

#[async_trait]
impl KafkaHandler<CartEventEnvelope> for CartsEventsKafkaHandler {
    const PATTERN: &'static str = "carts.events";
    async fn handle(&self, msg: KafkaMessage<CartEventEnvelope>) -> Result<(), KafkaError> {
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

ferrite_kafka::submit_kafka_handler!(CartsEventsKafkaHandler, CartEventEnvelope);

#[allow(dead_code)]
pub fn __force_link_cqrs_buses(_: &CommandBus, _: &QueryBus, _: &EventBus) {}
