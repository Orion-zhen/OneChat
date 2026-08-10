use gpui::Window;

use super::super::OneChat;
use crate::domain::{
    AssistantResponse, Conversation, Message, Model, Provider, RequestInfo, SystemPromptPreset,
    Turn, active_turns, user_branches,
};

impl OneChat {
    pub(crate) fn current_animated_title(&mut self, window: &mut Window) -> Option<String> {
        let conversation_id = self.current_conversation()?.id.clone();
        let (title, finished) = self.chat.title_transitions.get(&conversation_id)?.frame();
        if finished {
            self.chat.title_transitions.remove(&conversation_id);
        } else {
            window.request_animation_frame();
        }
        Some(title)
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

    pub(crate) fn prompt_preset(&self, name: &str) -> Option<&SystemPromptPreset> {
        self.data
            .snapshot
            .prompt_presets
            .iter()
            .find(|preset| preset.name == name)
    }

    pub(crate) fn prompt_preset_for_content(&self, content: &str) -> Option<&SystemPromptPreset> {
        let content = content.trim();
        self.settings()
            .default_system_prompt_preset
            .as_deref()
            .and_then(|name| self.prompt_preset(name))
            .filter(|preset| preset.content == content)
            .or_else(|| {
                self.data
                    .snapshot
                    .prompt_presets
                    .iter()
                    .find(|preset| preset.content == content)
            })
    }

    pub(crate) fn system_prompt_label(&self, content: &str) -> String {
        if content.trim().is_empty() {
            "None".into()
        } else {
            self.prompt_preset_for_content(content)
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

    pub(crate) fn current_context_messages(&self) -> Vec<Message> {
        crate::application::generation::history_for_new_turn(&self.data.snapshot.current_turns)
    }
}
