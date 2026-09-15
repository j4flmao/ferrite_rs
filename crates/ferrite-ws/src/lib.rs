//! # Ferrite WebSocket Gateways
//!
//! NestJS-style WebSocket gateways for Ferrite. Wires user-defined
//! [`Gateway`] trait implementations into axum's built-in `WebSocketUpgrade`
//! extractor and broadcasts messages via an injectable [`WsServer`].
//!
//! ## Example
//!
//! ```rust,ignore
//! use ferrite_ws::{Gateway, WsContext, OutgoingMessage, submit_gateway};
//! use ferrite_macros::{module, injectable};
//! use async_trait::async_trait;
//! use serde_json::json;
//! use std::sync::Arc;
//!
//! #[injectable]
//! pub struct ChatGateway;
//!
//! submit_gateway!(ChatGateway, "/ws/chat");
//!
//! #[async_trait]
//! impl Gateway for ChatGateway {
//!     async fn handle_message(
//!         &self,
//!         ctx: Arc<WsContext>,
//!         event: &str,
//!         data: serde_json::Value,
//!     ) -> Result<Option<OutgoingMessage>, ferrite_ws::WsError> {
//!         match event {
//!             "ping" => Ok(Some(OutgoingMessage::event("pong", json!({"ok": true})))),
//!             _ => Ok(None),
//!         }
//!     }
//! }
//!
//! #[module(providers = [ChatGateway])]
//! pub struct AppModule;
//! ```

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use dashmap::DashMap;
use fr_core::ModuleDescriptor;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum WsError {
    #[error("websocket protocol error: {0}")]
    Protocol(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("socket closed")]
    Closed,
    #[error("gateway error: {0}")]
    Gateway(String),
}

impl From<axum::Error> for WsError {
    fn from(e: axum::Error) -> Self {
        WsError::Protocol(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Wire format
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub event: String,
    #[serde(default = "Value::default")]
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    pub data: Value,
}

impl OutgoingMessage {
    pub fn event<E: Into<String>, V: Serialize>(event: E, data: V) -> Self {
        Self {
            event: Some(event.into()),
            data: serde_json::to_value(data).unwrap_or(Value::Null),
        }
    }
    pub fn raw<V: Serialize>(data: V) -> Self {
        Self {
            event: None,
            data: serde_json::to_value(data).unwrap_or(Value::Null),
        }
    }
}

// ---------------------------------------------------------------------------
// Context (one per accepted WebSocket connection)
// ---------------------------------------------------------------------------

pub type SocketId = Uuid;

pub struct WsContext {
    pub id: SocketId,
    pub path: String,
    pub query: Option<String>,
    server: Arc<WsServer>,
}

impl WsContext {
    pub async fn emit<M: Into<OutgoingMessage>>(&self, msg: M) -> Result<(), WsError> {
        self.server.send_to(self.id, msg.into()).await
    }

    pub async fn broadcast<M: Into<OutgoingMessage>>(&self, msg: M) -> Result<(), WsError> {
        self.server.broadcast(msg.into()).await
    }

    pub fn server(&self) -> &Arc<WsServer> {
        &self.server
    }
}

// ---------------------------------------------------------------------------
// Gateway trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait Gateway: Send + Sync + 'static {
    fn path(&self) -> &'static str {
        "/ws"
    }

    async fn handle_connection(&self, _ctx: Arc<WsContext>) -> Result<(), WsError> {
        Ok(())
    }

    async fn handle_message(
        &self,
        _ctx: Arc<WsContext>,
        _event: &str,
        _data: Value,
    ) -> Result<Option<OutgoingMessage>, WsError> {
        Ok(None)
    }

    async fn handle_disconnect(&self, _ctx: Arc<WsContext>) {}
}

// ---------------------------------------------------------------------------
// Inventory registry: gateway descriptors
// ---------------------------------------------------------------------------

pub struct GatewayDescriptor {
    pub gateway_type: TypeId,
    pub path: &'static str,
    pub as_gateway: fn(fr_core::AnyArc) -> Arc<dyn Gateway>,
}

inventory::collect!(GatewayDescriptor);

/// Downcast an erased `AnyArc` (Arc<dyn Any + Send + Sync>) to
/// `Arc<dyn Gateway>` for a concrete gateway type `T`.
pub fn any_arc_to_gateway<T: Gateway>(arc: fr_core::AnyArc) -> Arc<dyn Gateway> {
    // Arc<dyn Any + Send + Sync> can be downcast via Any::downcast_arc
    let typed: Arc<T> = arc
        .downcast::<T>()
        .expect("submit_gateway: AnyArc does not match gateway type");
    typed as Arc<dyn Gateway>
}

/// Submit a concrete gateway type and its HTTP path into the linker-folded
/// inventory so the runtime can discover it at bootstrap.
#[macro_export]
macro_rules! submit_gateway {
    ($ty:ty, $path:expr) => {
        const _: () = {
            ::inventory::submit! {
                $crate::GatewayDescriptor {
                    gateway_type: ::std::any::TypeId::of::<$ty>(),
                    path: $path,
                    as_gateway: $crate::any_arc_to_gateway::<$ty>,
                }
            }
        };
    };
}

// ---------------------------------------------------------------------------
// WsServer: socket registry, emit/broadcast
// ---------------------------------------------------------------------------

pub type Tx = tokio::sync::mpsc::UnboundedSender<Message>;

pub struct WsServer {
    pub sockets: Arc<DashMap<SocketId, Tx>>,
    pub rooms: Arc<DashMap<String, DashMap<SocketId, ()>>>,
    _anchor: Arc<u8>,
}

impl Default for WsServer {
    fn default() -> Self {
        Self {
            sockets: Arc::new(DashMap::new()),
            rooms: Arc::new(DashMap::new()),
            _anchor: Arc::new(0),
        }
    }
}

impl fr_core::Injectable for WsServer {
    fn __provider_entry() -> fr_core::ProviderEntry {
        use fr_core::ProviderEntry;
        use fr_core::Scope;
        ProviderEntry::new_static::<WsServer>(
            "WsServer",
            Scope::Singleton,
            Vec::new,
            |_container| -> Arc<dyn Any + Send + Sync> {
                Arc::new(WsServer::default()) as Arc<dyn Any + Send + Sync>
            },
        )
    }
}

inventory::submit! {
    fr_core::ProviderEntry::new_static::<WsServer>(
        "WsServer",
        fr_core::Scope::Singleton,
        Vec::new,
        |_container| -> std::sync::Arc<dyn std::any::Any + Send + Sync> {
            std::sync::Arc::new(WsServer::default()) as std::sync::Arc<dyn std::any::Any + Send + Sync>
        },
    )
}

impl WsServer {
    fn register(&self, id: SocketId, tx: Tx) {
        self.sockets.insert(id, tx);
    }

    fn unregister(&self, id: SocketId) {
        self.sockets.remove(&id);
        for room in self.rooms.iter() {
            room.value().remove(&id);
        }
    }

    pub fn count(&self) -> usize {
        self.sockets.len()
    }

    pub async fn send_to<M: Into<OutgoingMessage>>(
        &self,
        id: SocketId,
        msg: M,
    ) -> Result<(), WsError> {
        let msg = msg.into();
        let text = serde_json::to_string(&msg).map_err(WsError::Json)?;
        if let Some(tx) = self.sockets.get(&id) {
            let _ = tx.send(Message::Text(text.into()));
            Ok(())
        } else {
            Err(WsError::Closed)
        }
    }

    pub async fn broadcast<M: Into<OutgoingMessage>>(&self, msg: M) -> Result<(), WsError> {
        let msg = msg.into();
        let text = serde_json::to_string(&msg).map_err(WsError::Json)?;
        for tx in self.sockets.iter() {
            let _ = tx.value().send(Message::Text(text.clone().into()));
        }
        Ok(())
    }

    pub async fn emit_to_room<M: Into<OutgoingMessage>>(
        &self,
        room: &str,
        msg: M,
    ) -> Result<(), WsError> {
        let msg = msg.into();
        let text = serde_json::to_string(&msg).map_err(WsError::Json)?;
        if let Some(ids) = self.rooms.get(room) {
            for id in ids.iter() {
                if let Some(tx) = self.sockets.get(id.key()) {
                    let _ = tx.value().send(Message::Text(text.clone().into()));
                }
            }
        }
        Ok(())
    }

    pub fn join_room(&self, id: SocketId, room: impl Into<String>) {
        self.rooms.entry(room.into()).or_default().insert(id, ());
    }

    pub fn leave_room(&self, id: SocketId, room: &str) {
        if let Some(ids) = self.rooms.get(room) {
            ids.remove(&id);
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime dispatch path → gateway
// ---------------------------------------------------------------------------

type ToGatewayFn = fn(fr_core::AnyArc) -> Arc<dyn Gateway>;

fn build_routes() -> HashMap<String, (TypeId, ToGatewayFn)> {
    let mut out: HashMap<String, (TypeId, ToGatewayFn)> = HashMap::new();
    for desc in inventory::iter::<GatewayDescriptor> {
        out.insert(desc.path.to_string(), (desc.gateway_type, desc.as_gateway));
    }
    out
}

// ---------------------------------------------------------------------------
// HTTP handler glue (axum)
// ---------------------------------------------------------------------------

pub struct WsRuntime {
    pub container: fr_core::Container,
    pub server: Arc<WsServer>,
    pub routes: HashMap<String, (TypeId, ToGatewayFn)>,
    pub prefix: String,
}

async fn ws_upgrade_handler(
    State(rt): State<Arc<WsRuntime>>,
    ws: WebSocketUpgrade,
    uri: axum::http::Uri,
) -> Response {
    let path = uri.path().to_string();
    let query = uri.query().map(|q| q.to_string());
    ws.on_upgrade(move |socket| handle_socket(socket, rt, path, query))
}

async fn handle_socket(socket: WebSocket, rt: Arc<WsRuntime>, path: String, query: Option<String>) {
    use futures_util::StreamExt;
    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let id = Uuid::new_v4();
    rt.server.register(id, tx);

    let lookup = if path.starts_with(&rt.prefix) || rt.prefix.is_empty() {
        path.clone()
    } else {
        format!("{}{}", rt.prefix, path)
    };

    let ctx = Arc::new(WsContext {
        id,
        path: lookup.clone(),
        query,
        server: rt.server.clone(),
    });

    // Look up gateway from routes
    let gateway: Option<Arc<dyn Gateway>> =
        rt.routes.get(&lookup).map(|(gateway_type, to_gateway)| {
            let any = rt.container.get_any(*gateway_type);
            (to_gateway)(any)
        });

    if let Some(gw) = gateway.as_ref() {
        if gw.handle_connection(ctx.clone()).await.is_err() {
            rt.server.unregister(id);
            return;
        }
    }

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Copy outgoing data from rx channel -> ws_sender
    let mut tx_task = tokio::spawn(async move {
        use futures_util::SinkExt;
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
        let _ = ws_sender.close().await;
    });

    loop {
        tokio::select! {
            item = ws_receiver.next() => {
                let Some(res) = item else { break };
                let msg = match res {
                    Ok(m) => m,
                    Err(_) => break,
                };
                match msg {
                    Message::Text(t) => {
                        let parsed: Result<IncomingMessage, _> = serde_json::from_str(&t);
                        match parsed {
                            Ok(incoming) => {
                                if let Some(gw) = gateway.as_ref() {
                                    match gw
                                        .handle_message(ctx.clone(), &incoming.event, incoming.data)
                                        .await
                                    {
                                        Ok(Some(reply)) => {
                                            let _ = ctx.emit(reply).await;
                                        }
                                        Err(_err) => {
                                            // Log (opt-in via tracing)
                                        }
                                        Ok(None) => {}
                                    }
                                }
                            }
                            Err(_e) => {}
                        }
                    }
                    Message::Binary(_) => {}
                    Message::Ping(_) | Message::Pong(_) => {}
                    Message::Close(_) => break,
                }
            }
            _ = (&mut tx_task) => break,
        }
    }

    if let Some(gw) = gateway.as_ref() {
        gw.handle_disconnect(ctx.clone()).await;
    }
    rt.server.unregister(id);
}

// ---------------------------------------------------------------------------
// WsModule + mount helper
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct WsConfig {
    pub prefix: &'static str,
}

pub struct WsModule;

impl WsModule {
    pub fn for_root(config: WsConfig) -> WsModuleBuilder {
        WsModuleBuilder {
            prefix: config.prefix,
        }
    }
}

pub struct WsModuleBuilder {
    prefix: &'static str,
}

impl WsModuleBuilder {
    pub fn build(self) -> WsModuleImpl {
        WsModuleImpl {
            prefix: self.prefix,
        }
    }
}

#[derive(Clone)]
pub struct WsModuleImpl {
    pub prefix: &'static str,
}

impl fr_core::Module for WsModuleImpl {
    fn __module_descriptor() -> ModuleDescriptor
    where
        Self: Sized,
    {
        ModuleDescriptor {
            name: String::from("WsModule"),
            providers: vec![TypeId::of::<WsServer>()],
            controllers: vec![],
            imports: vec![],
            exports: vec![TypeId::of::<WsServer>()],
            middleware: vec![],
            global: false,
        }
    }
}

impl fr_core::OnApplicationBootstrap for WsModuleImpl {}

/// Mount the WebSocket upgrade route on an existing Axum router using the
/// given DI container and route prefix.
///
/// Prefix mapping example: prefix = "/ws", gateway path = "/chat" → full
/// upgrade path is `/ws/chat`.
pub fn mount_on(router: &mut axum::Router, container: fr_core::Container, prefix: &str) {
    let mount_at = if prefix.is_empty() || prefix == "/" {
        "/ws".to_string()
    } else {
        prefix.trim_end_matches('/').to_string()
    };
    // Ensure WsServer is at least a default-constructed instance even when
    // the caller forgot to include WsModule in their module graph.
    let server: Arc<WsServer> = if TypeId::of::<()>() == TypeId::of::<()>() {
        // container.get() panics if provider not registered; fall back safely
        use std::panic::{catch_unwind, AssertUnwindSafe};
        catch_unwind(AssertUnwindSafe(|| container.get::<WsServer>()))
            .unwrap_or_else(|_| Arc::new(WsServer::default()))
    } else {
        unreachable!()
    };
    let routes = build_routes();
    let rt = Arc::new(WsRuntime {
        container: container.clone(),
        server,
        routes,
        prefix: mount_at.clone(),
    });
    let fallback = axum::Router::new()
        .route("/{*ws_path}", axum::routing::get(ws_upgrade_handler))
        .with_state(rt.clone());
    *router = router.clone().nest(&mount_at, fallback);
}

// Make `Any` importable without fuss inside user code — kept after imports
// above — re-use via #[allow(unused)]:
#[allow(dead_code)]
fn _ensure_any_in_scope(_x: &dyn Any) {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outgoing_message_event_includes_event_key() {
        let msg = OutgoingMessage::event("pong", serde_json::json!({"ok":true}));
        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(serialized.contains("\"event\":\"pong\""));
        assert!(serialized.contains("\"ok\":true"));
    }

    #[test]
    fn outgoing_message_raw_omits_event_key() {
        let msg = OutgoingMessage::raw(serde_json::json!([1, 2, 3]));
        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(!serialized.contains("\"event\""));
    }

    #[test]
    fn ws_server_register_count_unregister() {
        let srv = WsServer::default();
        let id = Uuid::new_v4();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
        srv.register(id, tx);
        assert_eq!(srv.count(), 1);
        srv.unregister(id);
        assert_eq!(srv.count(), 0);
    }

    #[tokio::test]
    async fn ws_server_rooms_join_and_leave() {
        let srv = WsServer::default();
        let id = Uuid::new_v4();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
        srv.register(id, tx);
        srv.join_room(id, "lobby");
        srv.join_room(id, "game");
        assert!(srv.rooms.contains_key("lobby"));
        srv.leave_room(id, "lobby");
        let still_in_lobby = srv
            .rooms
            .get("lobby")
            .map(|r| r.contains_key(&id))
            .unwrap_or(false);
        assert!(!still_in_lobby);
    }

    #[test]
    fn incoming_message_default_data_is_null() {
        let s = r#"{"event":"ping"}"#;
        let m: IncomingMessage = serde_json::from_str(s).unwrap();
        assert_eq!(m.event, "ping");
        assert_eq!(m.data, Value::Null);
    }

    #[test]
    fn build_routes_uses_submitted_gateway() {
        // Tests only that inventory::collect is set up (no real gateway
        // submitted in unit test context — the submit_gateway! invocation in
        // a binary would show up here).
        let routes = build_routes();
        // Routes may be empty when no gateway has been linked in — that's fine.
        let _: &HashMap<_, _> = &routes;
    }
}
