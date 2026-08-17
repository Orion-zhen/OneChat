use std::collections::HashMap;

use crate::domain::{AssistantResponse, Turn};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationSearchSource {
    User,
    Assistant,
}

#[derive(Clone, Debug)]
pub struct ConversationSearchEntry {
    pub turn_id: String,
    pub response_id: Option<String>,
    pub source: ConversationSearchSource,
    pub content: String,
    normalized: String,
}

impl ConversationSearchEntry {
    fn user(turn: &Turn) -> Self {
        Self {
            turn_id: turn.id.clone(),
            response_id: None,
            source: ConversationSearchSource::User,
            content: turn.user.content.clone(),
            normalized: turn.user.content.to_lowercase(),
        }
    }

    fn assistant(turn_id: &str, response: &AssistantResponse) -> Self {
        Self {
            turn_id: turn_id.to_string(),
            response_id: Some(response.id.clone()),
            source: ConversationSearchSource::Assistant,
            content: response.content.clone(),
            normalized: response.content.to_lowercase(),
        }
    }

    pub fn matches_normalized(&self, normalized_query: &str) -> bool {
        self.normalized.contains(normalized_query)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ConversationSearchIndex {
    entries: HashMap<String, Vec<ConversationSearchEntry>>,
}

impl ConversationSearchIndex {
    pub(super) fn insert_conversation(&mut self, conversation_id: String, turns: &[Turn]) {
        let mut entries = Vec::new();
        for turn in turns {
            entries.push(ConversationSearchEntry::user(turn));
            entries.extend(
                turn.responses
                    .iter()
                    .map(|response| ConversationSearchEntry::assistant(&turn.id, response)),
            );
        }
        self.entries.insert(conversation_id, entries);
    }

    pub fn entries(&self, conversation_id: &str) -> &[ConversationSearchEntry] {
        self.entries
            .get(conversation_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(crate) fn update_assistant_response(
        &mut self,
        conversation_id: &str,
        turn_id: &str,
        response: &AssistantResponse,
    ) {
        let entries = self.entries.entry(conversation_id.to_string()).or_default();
        let entry = ConversationSearchEntry::assistant(turn_id, response);
        if let Some(stored) = entries
            .iter_mut()
            .find(|stored| stored.response_id.as_deref() == Some(response.id.as_str()))
        {
            *stored = entry;
        } else {
            entries.push(entry);
        }
    }
}
