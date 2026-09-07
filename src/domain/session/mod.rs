use serde::{Deserialize, Serialize};

use super::{AutoTitleState, Conversation, RequestInfo, Turn};

mod branches;
mod generation;

pub use generation::GenerationStart;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ConversationSession {
    #[serde(flatten)]
    pub conversation: Conversation,
    pub turns: Vec<Turn>,
    #[serde(default)]
    pub requests: Vec<RequestInfo>,
}

impl ConversationSession {
    pub fn new(conversation: Conversation) -> Self {
        Self {
            conversation,
            turns: Vec::new(),
            requests: Vec::new(),
        }
    }

    pub fn update_conversation(&mut self, conversation: &Conversation) {
        let title = self.conversation.title.clone();
        let auto_title_state = self.conversation.auto_title_state;
        let updated_at = self.conversation.updated_at.max(conversation.updated_at);
        self.conversation = conversation.clone();
        self.conversation.title = title;
        self.conversation.auto_title_state = auto_title_state;
        self.conversation.updated_at = updated_at;
    }

    pub fn rename(&mut self, title: &str) -> Result<(), String> {
        let title = title.trim();
        if title.is_empty() {
            return Err("conversation title cannot be empty".into());
        }
        self.conversation.title = title.to_string();
        self.conversation.auto_title_state = AutoTitleState::Finished;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.turns.clear();
        self.requests.clear();
    }

    fn turn_index(&self, id: &str) -> Result<usize, String> {
        self.turns
            .iter()
            .position(|turn| turn.id == id)
            .ok_or_else(|| format!("turn not found: {id}"))
    }
}
