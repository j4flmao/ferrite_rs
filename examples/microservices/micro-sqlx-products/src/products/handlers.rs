use crate::products::dto::{
    CreateProductCommand, GetProductQuery, ListProductsQuery, Product, ProductCreatedEvent,
    ProductList, StockUpdatedEvent, UpdateProductStockCommand,
};
use crate::products::repo::ProductsRepo;
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

ferrite_cqrs::__private_shim_command!(CreateProductHandler, CreateProductCommand);
ferrite_cqrs::__private_shim_command!(UpdateProductStockHandler, UpdateProductStockCommand);
ferrite_cqrs::__private_shim_query!(GetProductHandler, GetProductQuery);
ferrite_cqrs::__private_shim_query!(ListProductsHandler, ListProductsQuery);
ferrite_cqrs::__private_shim_event!(ProductCreatedEventHandler, ProductCreatedEvent);
ferrite_cqrs::__private_shim_event!(AuditProductEventsHandler, ProductCreatedEvent);

// ========== Command Handlers ==========
#[injectable]
pub struct CreateProductHandler {
    repo: ProductsRepo,
    events: EventBus,
}

impl CreateProductHandler {
    #[inject]
    pub fn new(repo: ProductsRepo, events: EventBus) -> Self {
        Self { repo, events }
    }
}

#[async_trait]
impl CommandHandler<CreateProductCommand> for CreateProductHandler {
    async fn handle(&self, cmd: CreateProductCommand) {
        let now = chrono::Utc::now().to_rfc3339();
        let product = Product {
            id: cmd.id,
            title: cmd.title,
            description: cmd.description,
            price: cmd.price,
            stock: cmd.stock,
            created_at: now.clone(),
            updated_at: now,
        };
        let event = ProductCreatedEvent {
            product: product.clone(),
            at: chrono::Utc::now().to_rfc3339(),
        };
        self.repo.insert(product).await;
        let _ = self.events.publish(event).await;
    }
}

submit_command_handler!(CreateProductHandler, CreateProductCommand);

#[injectable]
pub struct UpdateProductStockHandler {
    repo: ProductsRepo,
    events: EventBus,
    publisher: KafkaProductPublisher,
}

impl UpdateProductStockHandler {
    #[inject]
    pub fn new(repo: ProductsRepo, events: EventBus, publisher: KafkaProductPublisher) -> Self {
        Self {
            repo,
            events,
            publisher,
        }
    }
}

#[async_trait]
impl CommandHandler<UpdateProductStockCommand> for UpdateProductStockHandler {
    async fn handle(&self, cmd: UpdateProductStockCommand) {
        if let Some((old, new, _prod)) = self.repo.update_stock(&cmd.product_id, cmd.delta).await {
            let ev = StockUpdatedEvent {
                product_id: cmd.product_id,
                old_stock: old,
                new_stock: new,
                delta: cmd.delta,
                at: chrono::Utc::now().to_rfc3339(),
            };
            let ev_clone = ev.clone();
            let _ = self.events.publish(ev).await;
            let _ = self.publisher.publish_stock_updated(&ev_clone).await;
        }
    }
}

submit_command_handler!(UpdateProductStockHandler, UpdateProductStockCommand);

// ========== Query Handlers ==========
#[injectable]
pub struct GetProductHandler {
    repo: ProductsRepo,
}

impl GetProductHandler {
    #[inject]
    pub fn new(repo: ProductsRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl QueryHandler<GetProductQuery> for GetProductHandler {
    async fn handle(&self, q: GetProductQuery) -> Option<Product> {
        self.repo.get(&q.product_id).await
    }
}

submit_query_handler!(GetProductHandler, GetProductQuery);

#[injectable]
pub struct ListProductsHandler {
    repo: ProductsRepo,
}

impl ListProductsHandler {
    #[inject]
    pub fn new(repo: ProductsRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl QueryHandler<ListProductsQuery> for ListProductsHandler {
    async fn handle(&self, q: ListProductsQuery) -> ProductList {
        let (total, items) = self.repo.list(q.limit, q.offset).await;
        ProductList {
            total,
            limit: q.limit,
            offset: q.offset,
            items,
        }
    }
}

submit_query_handler!(ListProductsHandler, ListProductsQuery);

// ========== Event Handlers (fan-out) ==========
#[injectable]
pub struct ProductCreatedEventHandler {
    publisher: KafkaProductPublisher,
}

impl ProductCreatedEventHandler {
    #[inject]
    pub fn new(publisher: KafkaProductPublisher) -> Self {
        Self { publisher }
    }
}

#[async_trait]
impl EventHandler<ProductCreatedEvent> for ProductCreatedEventHandler {
    async fn handle(&self, ev: ProductCreatedEvent) {
        let _ = self.publisher.publish_created(&ev).await;
    }
}

submit_event_handler!(ProductCreatedEventHandler, ProductCreatedEvent);

#[injectable]
pub struct AuditProductEventsHandler {
    log: Mutex<Vec<String>>,
}

impl AuditProductEventsHandler {
    #[inject]
    pub fn new() -> Self {
        Self {
            log: Mutex::new(Vec::new()).into(),
        }
    }
}

#[async_trait]
impl EventHandler<ProductCreatedEvent> for AuditProductEventsHandler {
    async fn handle(&self, ev: ProductCreatedEvent) {
        let mut g = self.log.lock().await;
        g.push(format!(
            "[{}] PRODUCT_CREATED id={} title={:?} price={}",
            ev.at, ev.product.id, ev.product.title, ev.product.price
        ));
        let cap = g.len();
        if cap > 1000 {
            let drop_n = cap - 1000;
            g.drain(0..drop_n);
        }
    }
}

submit_event_handler!(AuditProductEventsHandler, ProductCreatedEvent);

// ========== Kafka publisher + pattern consumer ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductEventEnvelope {
    pub kind: String,
    pub id: String,
    pub payload: serde_json::Value,
}

#[injectable]
pub struct KafkaProductPublisher {
    kafka: KafkaServer,
}

impl KafkaProductPublisher {
    #[inject]
    pub fn new(kafka: KafkaServer) -> Self {
        Self { kafka }
    }

    pub async fn publish_created(&self, ev: &ProductCreatedEvent) -> Result<(), String> {
        let env = ProductEventEnvelope {
            kind: "product.created".into(),
            id: ev.product.id.clone(),
            payload: serde_json::to_value(&ev.product).map_err(|e| e.to_string())?,
        };
        self.kafka
            .produce("products.events", Some(ev.product.id.as_str()), &env)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn publish_stock_updated(&self, ev: &StockUpdatedEvent) -> Result<(), String> {
        let env = ProductEventEnvelope {
            kind: "product.stock_updated".into(),
            id: ev.product_id.clone(),
            payload: serde_json::to_value(ev).map_err(|e| e.to_string())?,
        };
        self.kafka
            .produce("products.events", Some(ev.product_id.as_str()), &env)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[injectable]
pub struct ProductsEventsKafkaHandler {
    processed: Mutex<Vec<String>>,
}

impl ProductsEventsKafkaHandler {
    #[inject]
    pub fn new() -> Self {
        Self {
            processed: Mutex::new(Vec::new()).into(),
        }
    }
}

#[async_trait]
impl KafkaHandler<ProductEventEnvelope> for ProductsEventsKafkaHandler {
    const PATTERN: &'static str = "products.events";
    async fn handle(&self, msg: KafkaMessage<ProductEventEnvelope>) -> Result<(), KafkaError> {
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

ferrite_kafka::submit_kafka_handler!(ProductsEventsKafkaHandler, ProductEventEnvelope);

#[allow(dead_code)]
pub fn __force_link_cqrs_buses(_: &CommandBus, _: &QueryBus, _: &EventBus) {}
