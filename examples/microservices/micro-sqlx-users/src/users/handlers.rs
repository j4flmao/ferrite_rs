use crate::users::dto::{
    ChangePasswordCommand, CreateUserCommand, DeleteUserCommand, GetUserQuery, ListUsersQuery,
    PasswordChangedEvent, UpdateUserCommand, User, UserCreatedEvent, UserDeletedEvent,
    UserInternal, UserList, UserUpdatedEvent,
};
use crate::users::repo::UsersRepo;
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

ferrite_cqrs::__private_shim_command!(CreateUserHandler, CreateUserCommand);
ferrite_cqrs::__private_shim_command!(UpdateUserHandler, UpdateUserCommand);
ferrite_cqrs::__private_shim_command!(DeleteUserHandler, DeleteUserCommand);
ferrite_cqrs::__private_shim_command!(ChangePasswordHandler, ChangePasswordCommand);
ferrite_cqrs::__private_shim_query!(GetUserHandler, GetUserQuery);
ferrite_cqrs::__private_shim_query!(ListUsersHandler, ListUsersQuery);
ferrite_cqrs::__private_shim_event!(UserCreatedEventHandler, UserCreatedEvent);
ferrite_cqrs::__private_shim_event!(UserUpdatedEventHandler, UserUpdatedEvent);
ferrite_cqrs::__private_shim_event!(AuditUserEventsHandler, UserCreatedEvent);
ferrite_cqrs::__private_shim_event!(AuditUserEventsHandler, UserUpdatedEvent);
ferrite_cqrs::__private_shim_event!(AuditUserEventsHandler, UserDeletedEvent);
ferrite_cqrs::__private_shim_event!(PasswordChangedEventHandler, PasswordChangedEvent);

// ========== Command Handlers ==========
#[injectable]
pub struct CreateUserHandler {
    repo: UsersRepo,
    events: EventBus,
}

impl CreateUserHandler {
    #[inject]
    pub fn new(repo: UsersRepo, events: EventBus) -> Self {
        Self { repo, events }
    }
}

#[async_trait]
impl CommandHandler<CreateUserCommand> for CreateUserHandler {
    async fn handle(&self, cmd: CreateUserCommand) {
        let now = chrono::Utc::now().to_rfc3339();
        let internal = UserInternal {
            id: cmd.id,
            name: cmd.name,
            email: cmd.email,
            password_hash: cmd.password_hash,
            avatar_url: cmd.avatar_url,
            bio: cmd.bio,
            role: cmd.role,
            is_active: true,
            created_at: now.clone(),
            updated_at: now,
        };
        let user = self.repo.insert(internal).await;
        let event = UserCreatedEvent {
            user: user.clone(),
            at: chrono::Utc::now().to_rfc3339(),
        };
        let _ = self.events.publish(event).await;
    }
}

submit_command_handler!(CreateUserHandler, CreateUserCommand);

#[injectable]
pub struct UpdateUserHandler {
    repo: UsersRepo,
    events: EventBus,
    publisher: KafkaUserPublisher,
}

impl UpdateUserHandler {
    #[inject]
    pub fn new(repo: UsersRepo, events: EventBus, publisher: KafkaUserPublisher) -> Self {
        Self {
            repo,
            events,
            publisher,
        }
    }
}

#[async_trait]
impl CommandHandler<UpdateUserCommand> for UpdateUserHandler {
    async fn handle(&self, cmd: UpdateUserCommand) {
        if let Some((changes, user)) = self
            .repo
            .update(cmd.user_id, cmd.name, cmd.avatar_url, cmd.bio)
            .await
        {
            if !changes.is_empty() {
                let ev = UserUpdatedEvent {
                    user_id: cmd.user_id,
                    user: user.clone(),
                    changes,
                    at: chrono::Utc::now().to_rfc3339(),
                };
                let ev_clone = ev.clone();
                let _ = self.events.publish(ev).await;
                let _ = self.publisher.publish_updated(&ev_clone).await;
            }
        }
    }
}

submit_command_handler!(UpdateUserHandler, UpdateUserCommand);

#[injectable]
pub struct DeleteUserHandler {
    repo: UsersRepo,
    events: EventBus,
}

impl DeleteUserHandler {
    #[inject]
    pub fn new(repo: UsersRepo, events: EventBus) -> Self {
        Self { repo, events }
    }
}

#[async_trait]
impl CommandHandler<DeleteUserCommand> for DeleteUserHandler {
    async fn handle(&self, cmd: DeleteUserCommand) {
        if self.repo.delete(cmd.user_id).await.is_some() {
            let ev = UserDeletedEvent {
                user_id: cmd.user_id,
                at: chrono::Utc::now().to_rfc3339(),
            };
            let _ = self.events.publish(ev).await;
        }
    }
}

submit_command_handler!(DeleteUserHandler, DeleteUserCommand);

#[injectable]
pub struct ChangePasswordHandler {
    repo: UsersRepo,
    events: EventBus,
}

impl ChangePasswordHandler {
    #[inject]
    pub fn new(repo: UsersRepo, events: EventBus) -> Self {
        Self { repo, events }
    }
}

#[async_trait]
impl CommandHandler<ChangePasswordCommand> for ChangePasswordHandler {
    async fn handle(&self, cmd: ChangePasswordCommand) {
        if self
            .repo
            .change_password(cmd.user_id, &cmd.new_password_hash)
            .await
            .is_some()
        {
            let ev = PasswordChangedEvent {
                user_id: cmd.user_id,
                at: chrono::Utc::now().to_rfc3339(),
            };
            let _ = self.events.publish(ev).await;
        }
    }
}

submit_command_handler!(ChangePasswordHandler, ChangePasswordCommand);

// ========== Query Handlers ==========
#[injectable]
pub struct GetUserHandler {
    repo: UsersRepo,
}

impl GetUserHandler {
    #[inject]
    pub fn new(repo: UsersRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl QueryHandler<GetUserQuery> for GetUserHandler {
    async fn handle(&self, q: GetUserQuery) -> Option<User> {
        self.repo.get(q.user_id).await
    }
}

submit_query_handler!(GetUserHandler, GetUserQuery);

#[injectable]
pub struct ListUsersHandler {
    repo: UsersRepo,
}

impl ListUsersHandler {
    #[inject]
    pub fn new(repo: UsersRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl QueryHandler<ListUsersQuery> for ListUsersHandler {
    async fn handle(&self, q: ListUsersQuery) -> UserList {
        let (total, items) = self.repo.list(q.limit, q.offset).await;
        UserList {
            total,
            limit: q.limit,
            offset: q.offset,
            items,
        }
    }
}

submit_query_handler!(ListUsersHandler, ListUsersQuery);

// ========== Event Handlers (fan-out) ==========
#[injectable]
pub struct UserCreatedEventHandler {
    publisher: KafkaUserPublisher,
}

impl UserCreatedEventHandler {
    #[inject]
    pub fn new(publisher: KafkaUserPublisher) -> Self {
        Self { publisher }
    }
}

#[async_trait]
impl EventHandler<UserCreatedEvent> for UserCreatedEventHandler {
    async fn handle(&self, ev: UserCreatedEvent) {
        let _ = self.publisher.publish_created(&ev).await;
    }
}

submit_event_handler!(UserCreatedEventHandler, UserCreatedEvent);

#[injectable]
pub struct UserUpdatedEventHandler {
    publisher: KafkaUserPublisher,
}

impl UserUpdatedEventHandler {
    #[inject]
    pub fn new(publisher: KafkaUserPublisher) -> Self {
        Self { publisher }
    }
}

#[async_trait]
impl EventHandler<UserUpdatedEvent> for UserUpdatedEventHandler {
    async fn handle(&self, ev: UserUpdatedEvent) {
        let _ = self.publisher.publish_updated(&ev).await;
    }
}

submit_event_handler!(UserUpdatedEventHandler, UserUpdatedEvent);

#[injectable]
pub struct PasswordChangedEventHandler {}

impl PasswordChangedEventHandler {
    #[inject]
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl EventHandler<PasswordChangedEvent> for PasswordChangedEventHandler {
    async fn handle(&self, _ev: PasswordChangedEvent) {}
}

submit_event_handler!(PasswordChangedEventHandler, PasswordChangedEvent);

#[injectable]
pub struct AuditUserEventsHandler {
    log: Mutex<Vec<String>>,
}

impl AuditUserEventsHandler {
    #[inject]
    pub fn new() -> Self {
        Self {
            log: Mutex::new(Vec::new()).into(),
        }
    }
}

#[async_trait]
impl EventHandler<UserCreatedEvent> for AuditUserEventsHandler {
    async fn handle(&self, ev: UserCreatedEvent) {
        let mut g = self.log.lock().await;
        g.push(format!(
            "[{}] USER_CREATED id={} name={:?} email={:?} role={}",
            ev.at, ev.user.id, ev.user.name, ev.user.email, ev.user.role
        ));
        let cap = g.len();
        if cap > 1000 {
            let drop_n = cap - 1000;
            g.drain(0..drop_n);
        }
    }
}

submit_event_handler!(AuditUserEventsHandler, UserCreatedEvent);

#[async_trait]
impl EventHandler<UserUpdatedEvent> for AuditUserEventsHandler {
    async fn handle(&self, ev: UserUpdatedEvent) {
        let mut g = self.log.lock().await;
        g.push(format!(
            "[{}] USER_UPDATED id={} changes={:?}",
            ev.at, ev.user_id, ev.changes
        ));
        let cap = g.len();
        if cap > 1000 {
            let drop_n = cap - 1000;
            g.drain(0..drop_n);
        }
    }
}

submit_event_handler!(AuditUserEventsHandler, UserUpdatedEvent);

#[async_trait]
impl EventHandler<UserDeletedEvent> for AuditUserEventsHandler {
    async fn handle(&self, ev: UserDeletedEvent) {
        let mut g = self.log.lock().await;
        g.push(format!("[{}] USER_DELETED id={}", ev.at, ev.user_id));
        let cap = g.len();
        if cap > 1000 {
            let drop_n = cap - 1000;
            g.drain(0..drop_n);
        }
    }
}

submit_event_handler!(AuditUserEventsHandler, UserDeletedEvent);

// ========== Kafka publisher + pattern consumer ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEventEnvelope {
    pub kind: String,
    pub id: String,
    pub payload: serde_json::Value,
}

#[injectable]
pub struct KafkaUserPublisher {
    kafka: KafkaServer,
}

impl KafkaUserPublisher {
    #[inject]
    pub fn new(kafka: KafkaServer) -> Self {
        Self { kafka }
    }

    pub async fn publish_created(&self, ev: &UserCreatedEvent) -> Result<(), String> {
        let env = UserEventEnvelope {
            kind: "user.created".into(),
            id: ev.user.id.to_string(),
            payload: serde_json::to_value(&ev.user).map_err(|e| e.to_string())?,
        };
        self.kafka
            .produce("users.events", Some(ev.user.id.to_string().as_str()), &env)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn publish_updated(&self, ev: &UserUpdatedEvent) -> Result<(), String> {
        let env = UserEventEnvelope {
            kind: "user.updated".into(),
            id: ev.user_id.to_string(),
            payload: serde_json::to_value(ev).map_err(|e| e.to_string())?,
        };
        self.kafka
            .produce("users.events", Some(ev.user_id.to_string().as_str()), &env)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[injectable]
pub struct UsersEventsKafkaHandler {
    processed: Mutex<Vec<String>>,
}

impl UsersEventsKafkaHandler {
    #[inject]
    pub fn new() -> Self {
        Self {
            processed: Mutex::new(Vec::new()).into(),
        }
    }
}

#[async_trait]
impl KafkaHandler<UserEventEnvelope> for UsersEventsKafkaHandler {
    const PATTERN: &'static str = "users.events";
    async fn handle(&self, msg: KafkaMessage<UserEventEnvelope>) -> Result<(), KafkaError> {
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

ferrite_kafka::submit_kafka_handler!(UsersEventsKafkaHandler, UserEventEnvelope);

#[allow(dead_code)]
pub fn __force_link_cqrs_buses(_: &CommandBus, _: &QueryBus, _: &EventBus) {}
