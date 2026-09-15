use std::sync::Arc;

use async_trait::async_trait;
use ferrite_auth_jwt::JwtService;
use ferrite_framework::{inject, injectable};
use ferrite_ws::{Gateway, OutgoingMessage, WsContext, WsError};
use serde_json::Value;

use super::service::{ChatMessage, ChatService};
use crate::modules::auth::AuthService;

#[injectable]
pub struct ChatGateway {
    chat: ChatService,
    auth: AuthService,
    jwt: JwtService,
}

impl ChatGateway {
    #[inject]
    pub fn new(chat: ChatService, auth: AuthService, jwt: JwtService) -> Self {
        Self { chat, auth, jwt }
    }

    fn resolve_author(&self, data: &Value, query: &Option<String>) -> (i64, String, String) {
        let token_candidate = data
            .get("token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| query.as_ref().and_then(|q| extract_query_token(q)));
        if let Some(tok) = token_candidate {
            if let Ok(claims) = self.jwt.verify(&tok) {
                if let Some(u) = self.auth.resolve(claims.sub, &claims.email) {
                    return (u.id, u.email, u.name);
                }
            }
        }
        let suffix = format!("guest-{}", &uuid::Uuid::new_v4().to_string()[..6]);
        (0, format!("{suffix}@local"), suffix)
    }
}

#[async_trait]
impl Gateway for ChatGateway {
    fn path(&self) -> &'static str {
        "/ws/chat"
    }

    async fn handle_connection(&self, ctx: Arc<WsContext>) -> Result<(), WsError> {
        let history: Vec<ChatMessage> = self.chat.last_n(50);
        let msg =
            OutgoingMessage::event("chat:history", serde_json::json!({ "messages": history }));
        ctx.emit(msg).await.ok();
        let count = ctx.server().count();
        let announce = OutgoingMessage::event(
            "chat:system",
            serde_json::json!({ "text": format!("connected. online: {}", count) }),
        );
        ctx.emit(announce).await.ok();
        Ok(())
    }

    async fn handle_message(
        &self,
        ctx: Arc<WsContext>,
        event: &str,
        data: Value,
    ) -> Result<Option<OutgoingMessage>, WsError> {
        if event == "chat:send" {
            let text = data
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if text.is_empty() || text.chars().count() > 2000 {
                return Ok(Some(OutgoingMessage::event(
                    "chat:error",
                    serde_json::json!({ "message": "text required (1-2000 chars)" }),
                )));
            }
            let (user_id, email, name) = self.resolve_author(&data, &ctx.query);
            let saved = self.chat.append(user_id, email, name, text);
            let broadcast =
                OutgoingMessage::event("chat:message", serde_json::to_value(&saved).unwrap());
            ctx.broadcast(broadcast).await.ok();
            return Ok(None);
        }
        if event == "chat:ping" {
            return Ok(Some(OutgoingMessage::event(
                "chat:pong",
                serde_json::json!({ "ts": chrono::Utc::now().timestamp() }),
            )));
        }
        Ok(None)
    }
}

ferrite_ws::submit_gateway!(ChatGateway, "/ws/chat");

fn extract_query_token(query: &str) -> Option<String> {
    for pair in query.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next()?;
        if k == "token" {
            let v = it.next().unwrap_or("");
            let decoded = percent_decode(v);
            return Some(decoded);
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'+' {
            out.push(b' ');
            i += 1;
        } else if b == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
            } else {
                out.push(b);
                i += 1;
            }
        } else {
            out.push(b);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_default()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
