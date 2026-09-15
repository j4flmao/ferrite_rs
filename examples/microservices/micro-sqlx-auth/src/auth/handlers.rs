use crate::auth::dto::{RegisterDto, User, UsersRepo};
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

// ========== CQRS: Commands ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterUserCommand {
    pub id: String,
    pub name: String,
    pub email: String,
    pub password_hash: String,
    pub role: String,
}
impl ferrite_cqrs::Command for RegisterUserCommand {}

impl RegisterUserCommand {
    pub fn to_register_dto(&self, password: &str) -> RegisterDto {
        RegisterDto {
            name: self.name.clone(),
            email: self.email.clone(),
            password: password.to_string(),
            role: self.role.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginUserCommand {
    pub email: String,
    pub password: String,
    pub ip: Option<String>,
}
impl ferrite_cqrs::Command for LoginUserCommand {}

// ========== CQRS: Queries ==========
#[derive(Debug, Clone)]
pub struct GetUserQuery {
    pub user_id: i64,
}
impl ferrite_cqrs::Query for GetUserQuery {
    type Result = Option<User>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateTokenQuery {
    pub token: String,
}
impl ferrite_cqrs::Query for ValidateTokenQuery {
    type Result = Option<User>;
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ValidateTokenQueryResult {
    #[schema(example = "valid")]
    pub status: String,
    pub user: Option<User>,
}

// ========== CQRS: Events (fan-out) ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCreatedEvent {
    pub user: User,
    pub at: String,
}
impl ferrite_cqrs::Event for UserCreatedEvent {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLoggedInEvent {
    pub user_id: i64,
    pub at: String,
    pub ip: Option<String>,
}
impl ferrite_cqrs::Event for UserLoggedInEvent {}

// ========== Shim macros ==========
ferrite_cqrs::__private_shim_command!(RegisterUserHandler, RegisterUserCommand);
ferrite_cqrs::__private_shim_command!(LoginUserHandler, LoginUserCommand);
ferrite_cqrs::__private_shim_query!(GetUserQueryHandler, GetUserQuery);
ferrite_cqrs::__private_shim_query!(ValidateTokenQueryHandler, ValidateTokenQuery);
ferrite_cqrs::__private_shim_event!(UserCreatedEventHandler, UserCreatedEvent);
ferrite_cqrs::__private_shim_event!(AuditAuthEventsHandler, UserCreatedEvent);

// ========== Command Handlers ==========
#[injectable]
pub struct RegisterUserHandler {
    repo: UsersRepo,
    events: EventBus,
}

impl RegisterUserHandler {
    #[inject]
    pub fn new(repo: UsersRepo, events: EventBus) -> Self {
        Self { repo, events }
    }
}

#[async_trait]
impl CommandHandler<RegisterUserCommand> for RegisterUserHandler {
    async fn handle(&self, cmd: RegisterUserCommand) {
        if self.repo.email_exists(&cmd.email) {
            return;
        }
        let user = self
            .repo
            .create(&cmd.to_register_dto(""), &cmd.password_hash);
        let event = UserCreatedEvent {
            user: user.clone(),
            at: chrono::Utc::now().to_rfc3339(),
        };
        self.events.publish(event).await.ok();
    }
}

submit_command_handler!(RegisterUserHandler, RegisterUserCommand);

#[injectable]
pub struct LoginUserHandler {
    repo: UsersRepo,
    events: EventBus,
}

impl LoginUserHandler {
    #[inject]
    pub fn new(repo: UsersRepo, events: EventBus) -> Self {
        Self { repo, events }
    }
}

#[async_trait]
impl CommandHandler<LoginUserCommand> for LoginUserHandler {
    async fn handle(&self, cmd: LoginUserCommand) {
        if let Some(user) = self.repo.find_by_email(&cmd.email) {
            let ev = UserLoggedInEvent {
                user_id: user.id,
                at: chrono::Utc::now().to_rfc3339(),
                ip: cmd.ip,
            };
            let _ = self.events.publish(ev).await;
        }
    }
}

submit_command_handler!(LoginUserHandler, LoginUserCommand);

// ========== Query Handlers ==========
#[injectable]
pub struct GetUserQueryHandler {
    repo: UsersRepo,
}

impl GetUserQueryHandler {
    #[inject]
    pub fn new(repo: UsersRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl QueryHandler<GetUserQuery> for GetUserQueryHandler {
    async fn handle(&self, q: GetUserQuery) -> Option<User> {
        self.repo.find_by_id(q.user_id).await.map(|u| User {
            password_hash: String::new(),
            ..u
        })
    }
}

submit_query_handler!(GetUserQueryHandler, GetUserQuery);

#[injectable]
pub struct ValidateTokenQueryHandler {
    repo: UsersRepo,
    jwt: ferrite_auth_jwt::JwtService,
}

impl ValidateTokenQueryHandler {
    #[inject]
    pub fn new(repo: UsersRepo, jwt: ferrite_auth_jwt::JwtService) -> Self {
        Self { repo, jwt }
    }
}

#[async_trait]
impl QueryHandler<ValidateTokenQuery> for ValidateTokenQueryHandler {
    async fn handle(&self, q: ValidateTokenQuery) -> Option<User> {
        match self.jwt.verify(&q.token) {
            Ok(claims) => self.repo.find_by_id(claims.sub).await.map(|u| User {
                password_hash: String::new(),
                ..u
            }),
            Err(_) => None,
        }
    }
}

submit_query_handler!(ValidateTokenQueryHandler, ValidateTokenQuery);

// ========== Event Handlers (fan-out) ==========
#[injectable]
pub struct UserCreatedEventHandler {
    publisher: KafkaAuthPublisher,
}

impl UserCreatedEventHandler {
    #[inject]
    pub fn new(publisher: KafkaAuthPublisher) -> Self {
        Self { publisher }
    }
}

#[async_trait]
impl EventHandler<UserCreatedEvent> for UserCreatedEventHandler {
    async fn handle(&self, ev: UserCreatedEvent) {
        let _ = self.publisher.publish_user_created(&ev).await;
    }
}

submit_event_handler!(UserCreatedEventHandler, UserCreatedEvent);

#[injectable]
pub struct AuditAuthEventsHandler {
    log: Mutex<Vec<String>>,
}

impl AuditAuthEventsHandler {
    #[inject]
    pub fn new() -> Self {
        Self {
            log: Mutex::new(Vec::new()).into(),
        }
    }
}

#[async_trait]
impl EventHandler<UserCreatedEvent> for AuditAuthEventsHandler {
    async fn handle(&self, ev: UserCreatedEvent) {
        let mut g = self.log.lock().await;
        g.push(format!(
            "[{}] USER_CREATED id={} email={:?} role={}",
            ev.at, ev.user.id, ev.user.email, ev.user.role
        ));
        let cap = g.len();
        if cap > 1000 {
            let drop_n = cap - 1000;
            g.drain(0..drop_n);
        }
    }
}

submit_event_handler!(AuditAuthEventsHandler, UserCreatedEvent);

// ========== Kafka publisher + pattern consumer ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthEventEnvelope {
    pub kind: String,
    pub id: String,
    pub payload: serde_json::Value,
}

#[injectable]
pub struct KafkaAuthPublisher {
    kafka: KafkaServer,
}

impl KafkaAuthPublisher {
    #[inject]
    pub fn new(kafka: KafkaServer) -> Self {
        Self { kafka }
    }

    pub async fn publish_user_created(&self, ev: &UserCreatedEvent) -> Result<(), String> {
        let env = AuthEventEnvelope {
            kind: "user.created".into(),
            id: ev.user.id.to_string(),
            payload: serde_json::to_value(&ev.user).map_err(|e| e.to_string())?,
        };
        self.kafka
            .produce("auth.events", Some(ev.user.id.to_string().as_str()), &env)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn publish_user_logged_in(&self, ev: &UserLoggedInEvent) -> Result<(), String> {
        let env = AuthEventEnvelope {
            kind: "user.logged_in".into(),
            id: ev.user_id.to_string(),
            payload: serde_json::to_value(ev).map_err(|e| e.to_string())?,
        };
        self.kafka
            .produce("auth.events", Some(ev.user_id.to_string().as_str()), &env)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[injectable]
pub struct AuthEventsKafkaHandler {
    processed: Mutex<Vec<String>>,
}

impl AuthEventsKafkaHandler {
    #[inject]
    pub fn new() -> Self {
        Self {
            processed: Mutex::new(Vec::new()).into(),
        }
    }
}

#[async_trait]
impl KafkaHandler<AuthEventEnvelope> for AuthEventsKafkaHandler {
    const PATTERN: &'static str = "auth.events";
    async fn handle(&self, msg: KafkaMessage<AuthEventEnvelope>) -> Result<(), KafkaError> {
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

ferrite_kafka::submit_kafka_handler!(AuthEventsKafkaHandler, AuthEventEnvelope);

#[allow(dead_code)]
pub fn __force_link_cqrs_buses(_: &CommandBus, _: &QueryBus, _: &EventBus) {}
