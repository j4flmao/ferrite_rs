use ferrite_framework::{controller, impl_controller, inject, Json};

use super::service::{ChatMessage, ChatService};

#[controller("/messages")]
pub struct ChatController {
    chat: ChatService,
}

#[impl_controller]
impl ChatController {
    #[inject]
    pub fn new(chat: ChatService) -> Self {
        Self { chat }
    }

    #[get("/")]
    pub async fn list(&self) -> Json<Vec<ChatMessage>> {
        Json(self.chat.last_n(100))
    }
}
