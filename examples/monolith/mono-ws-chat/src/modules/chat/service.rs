use std::collections::VecDeque;
use std::sync::Mutex;

use chrono::Utc;
use ferrite_framework::{inject, injectable};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatMessage {
    #[schema(value_type = String, example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    #[schema(example = 1)]
    pub user_id: i64,
    #[schema(example = "alice@example.com")]
    pub email: String,
    #[schema(example = "Alice")]
    pub name: String,
    #[schema(example = "hello world")]
    pub text: String,
    #[schema(example = 1710000000)]
    pub ts: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SendMessageDto {
    #[schema(example = "hello world", min_length = 1, max_length = 2000)]
    pub text: String,
}

#[injectable]
pub struct ChatService {
    inner: Mutex<VecDeque<ChatMessage>>,
}

impl ChatService {
    #[inject]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(VecDeque::with_capacity(128)).into(),
        }
    }

    pub const MAX_HISTORY: usize = 100;

    pub fn push(&self, msg: ChatMessage) {
        let mut q = self.inner.lock().expect("chat history mutex");
        if q.len() >= Self::MAX_HISTORY {
            q.pop_front();
        }
        q.push_back(msg);
    }

    pub fn append(
        &self,
        user_id: i64,
        email: impl Into<String>,
        name: impl Into<String>,
        text: impl Into<String>,
    ) -> ChatMessage {
        let msg = ChatMessage {
            id: Uuid::new_v4(),
            user_id,
            email: email.into(),
            name: name.into(),
            text: text.into(),
            ts: Utc::now().timestamp(),
        };
        self.push(msg.clone());
        msg
    }

    pub fn history(&self) -> Vec<ChatMessage> {
        self.inner
            .lock()
            .expect("chat history mutex")
            .iter()
            .cloned()
            .collect()
    }

    pub fn last_n(&self, n: usize) -> Vec<ChatMessage> {
        let all = self.history();
        let start = all.len().saturating_sub(n);
        all[start..].to_vec()
    }
}
