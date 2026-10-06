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
        let content = response.output_text();
        Self {
            turn_id: turn_id.to_string(),
            response_id: Some(response.id.clone()),
            source: ConversationSearchSource::Assistant,
            normalized: content.to_lowercase(),
            content,
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
    pub(crate) fn insert_conversation(&mut self, conversation_id: String, turns: &[Turn]) {
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

    pub(super) fn remove_conversation(&mut self, conversation_id: &str) {
        self.entries.remove(conversation_id);
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
        let Some(entries) = self.entries.get_mut(conversation_id) else {
            return;
        };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Conversation, Model, Provider, ProviderKind, UserMessage};

    #[test]
    fn late_generation_updates_do_not_restore_discarded_search_entries() {
        let provider = Provider::new("Provider", ProviderKind::OpenAi);
        let model = Model::new(&provider.id, "model", "Model", provider.kind);
        let conversation = Conversation::new("Chat", Some(&model), "");
        let mut response = AssistantResponse::new(&model, &provider);
        response.append_output("first", 0);
        let turn = Turn::new(
            &conversation,
            None,
            UserMessage::new("question", Vec::new()),
            response.clone(),
        );
        let mut index = ConversationSearchIndex::default();
        index.insert_conversation(conversation.id.clone(), std::slice::from_ref(&turn));
        let output_id = response.output_blocks().next().unwrap().0.to_string();
        response
            .replace_editable_text(&[], &[(output_id, "updated".into())])
            .unwrap();
        index.update_assistant_response(&conversation.id, &turn.id, &response);
        assert!(
            index
                .entries(&conversation.id)
                .iter()
                .any(|entry| entry.content == "updated")
        );
        index.remove_conversation(&conversation.id);
        index.update_assistant_response(&conversation.id, &turn.id, &response);
        assert!(index.entries(&conversation.id).is_empty());
    }
}
