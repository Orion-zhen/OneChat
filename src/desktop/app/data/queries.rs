use std::collections::HashMap;

use gpui::Window;

use super::super::OneChat;
use crate::domain::{
    AssistantResponse, Conversation, HistoryLimit, Message, Model, PromptPreset, Provider,
    RequestInfo, Turn, active_turns, user_branches,
};

impl OneChat {
    pub(crate) fn animated_titles(&mut self, window: &mut Window) -> HashMap<String, String> {
        let mut finished = Vec::new();
        let titles = self
            .chat
            .title_transitions
            .iter()
            .map(|(conversation_id, transition)| {
                let (title, is_finished) = transition.frame();
                if is_finished {
                    finished.push(conversation_id.clone());
                }
                (conversation_id.clone(), title)
            })
            .collect();
        if finished.len() < self.chat.title_transitions.len() {
            window.request_animation_frame();
        }
        for conversation_id in finished {
            self.chat.title_transitions.remove(&conversation_id);
        }
        titles
    }

    pub(crate) fn current_conversation(&self) -> Option<&Conversation> {
        let id = self
            .data
            .snapshot
            .settings
            .current_conversation_id
            .as_deref()?;
        self.data
            .snapshot
            .conversations
            .iter()
            .find(|conversation| conversation.id == id)
    }

    pub(crate) fn prompt_preset(&self, name: &str) -> Option<&PromptPreset> {
        self.data
            .snapshot
            .prompt_presets
            .iter()
            .find(|preset| preset.name == name)
    }

    pub(crate) fn prompt_preset_for_setup(
        &self,
        system_prompt: &str,
        assistant_opening: &str,
    ) -> Option<&PromptPreset> {
        let matches = |preset: &&PromptPreset| {
            preset.system_prompt == system_prompt.trim()
                && preset.assistant_opening == assistant_opening.trim()
        };
        self.settings()
            .default_prompt_preset
            .as_deref()
            .and_then(|name| self.prompt_preset(name))
            .filter(matches)
            .or_else(|| self.data.snapshot.prompt_presets.iter().find(matches))
    }

    pub(crate) fn prompt_setup_label(&self, conversation: &Conversation) -> String {
        if conversation.system_prompt.trim().is_empty()
            && conversation.assistant_opening.trim().is_empty()
        {
            "None".into()
        } else {
            self.prompt_preset_for_setup(
                &conversation.system_prompt,
                &conversation.assistant_opening,
            )
            .map(|preset| preset.name.clone())
            .unwrap_or_else(|| "Custom".into())
        }
    }

    pub(crate) fn primary_model(&self) -> Option<&Model> {
        let model_id = self.data.snapshot.settings.primary_model_id.as_deref()?;
        self.data
            .snapshot
            .models
            .iter()
            .find(|model| model.id == model_id)
    }

    pub(crate) fn title_generation_model(&self) -> Option<&Model> {
        self.data
            .snapshot
            .settings
            .title_generation_model_id
            .as_deref()
            .and_then(|model_id| {
                self.data
                    .snapshot
                    .models
                    .iter()
                    .find(|model| model.id == model_id)
            })
            .or_else(|| self.primary_model())
    }

    pub(crate) fn current_model(&self) -> Option<&Model> {
        let conversation = self.current_conversation()?;
        conversation
            .model_id
            .as_deref()
            .and_then(|model_id| {
                self.data
                    .snapshot
                    .models
                    .iter()
                    .find(|model| model.id == model_id)
            })
            .or_else(|| self.primary_model())
    }

    pub(crate) fn selected_model(&self) -> Option<&Model> {
        self.current_model()
            .or_else(|| {
                let model_id = self.chat.draft_model_id.as_deref()?;
                self.data
                    .snapshot
                    .models
                    .iter()
                    .find(|model| model.id == model_id)
            })
            .or_else(|| self.primary_model())
    }

    pub(crate) fn current_provider(&self) -> Option<&Provider> {
        let provider_id = &self.current_model()?.provider_id;
        self.data
            .snapshot
            .providers
            .iter()
            .find(|provider| &provider.id == provider_id)
    }

    pub(crate) fn provider_for_model(&self, model: &Model) -> Option<&Provider> {
        self.data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == model.provider_id)
    }

    pub(crate) fn model_availability(&self, model: &Model) -> Result<(), &'static str> {
        let Some(provider) = self.provider_for_model(model) else {
            return Err("Missing provider");
        };
        if !provider.enabled {
            return Err("Provider disabled");
        }
        if !provider.streaming {
            return Err("Streaming disabled");
        }
        Ok(())
    }

    pub(crate) fn current_turns(&self) -> Vec<&Turn> {
        active_turns(&self.data.snapshot.current_turns)
    }

    pub(crate) fn active_leaf_turn(&self) -> Option<&Turn> {
        self.current_turns().last().copied()
    }

    pub(crate) fn user_branches(&self, turn: &Turn) -> Vec<&Turn> {
        user_branches(&self.data.snapshot.current_turns, turn)
    }

    pub(crate) fn current_request(&self) -> Option<&RequestInfo> {
        let conversation = self.current_conversation()?;
        if let Some(active) = self.chat.generations.active_request(&conversation.id) {
            return self
                .data
                .snapshot
                .current_requests
                .iter()
                .find(|request| request.id == active.request_id);
        }
        self.active_leaf_turn()
            .and_then(|turn| {
                turn.continuation_response_id
                    .as_deref()
                    .and_then(|id| turn.response(id))
                    .or_else(|| turn.responses.first())
            })
            .and_then(|response| self.request_for_response(response))
    }

    pub(crate) fn request_for_response(
        &self,
        response: &AssistantResponse,
    ) -> Option<&RequestInfo> {
        let request_id = response.request_id.as_deref()?;
        self.data
            .snapshot
            .current_requests
            .iter()
            .find(|request| request.id == request_id)
    }

    pub(crate) fn inspected_request(&self) -> Option<&RequestInfo> {
        self.chat
            .selected_request_id
            .as_deref()
            .and_then(|id| {
                self.data
                    .snapshot
                    .current_requests
                    .iter()
                    .find(|request| request.id == id)
            })
            .or_else(|| self.current_request())
    }

    pub(crate) fn visible_response<'a>(&self, turn: &'a Turn) -> Option<&'a AssistantResponse> {
        self.chat
            .visible_response_ids
            .get(&turn.id)
            .and_then(|id| turn.response(id))
            .or_else(|| {
                turn.continuation_response_id
                    .as_deref()
                    .and_then(|id| turn.response(id))
            })
            .or_else(|| turn.responses.first())
    }

    pub(crate) fn response(&self, response_id: &str) -> Option<(&Turn, &AssistantResponse)> {
        self.data
            .snapshot
            .current_turns
            .iter()
            .find_map(|turn| turn.response(response_id).map(|response| (turn, response)))
    }

    pub(crate) fn is_latest_turn(&self, turn_id: &str) -> bool {
        self.active_leaf_turn()
            .is_some_and(|turn| turn.id == turn_id)
    }

    pub(crate) fn effective_history_limit(&self, conversation: &Conversation) -> HistoryLimit {
        conversation.effective_history_limit(self.data.snapshot.settings.history_limit)
    }

    pub(crate) fn displayed_history_limit(&self) -> HistoryLimit {
        self.chat.history_limit_preview.unwrap_or_else(|| {
            self.current_conversation()
                .map(|conversation| self.effective_history_limit(conversation))
                .unwrap_or_default()
        })
    }

    pub(crate) fn current_context_messages(&self) -> Vec<Message> {
        let limit = self.displayed_history_limit();
        let mut messages = crate::application::generation::history_for_new_turn(
            &self.data.snapshot.current_turns,
            limit,
        );
        if let Some(opening) = self
            .current_conversation()
            .map(|conversation| conversation.assistant_opening.trim())
            .filter(|opening| !opening.is_empty())
        {
            messages.insert(0, Message::assistant(opening.to_string()));
        }
        messages
    }

    pub(crate) fn current_context_audio_duration_ms(&self) -> u64 {
        crate::application::generation::history_audio_duration_ms_for_new_turn(
            &self.data.snapshot.current_turns,
            self.displayed_history_limit(),
        )
    }
}
