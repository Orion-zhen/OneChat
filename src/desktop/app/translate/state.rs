use gpui::{Context, ScrollHandle, Window};
use tokio_util::sync::CancellationToken;

use super::controls::TranslationControls;
use crate::{
    desktop::app::OneChat,
    domain::{
        AssistantResponse, DEFAULT_TRANSLATION_SYSTEM_PROMPT, DEFAULT_TRANSLATION_USER_PROMPT,
        RequestInfo,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TranslationPromptKind {
    System,
    User,
}

impl TranslationPromptKind {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::System => "System Prompt",
            Self::User => "User Prompt",
        }
    }
}

pub(crate) struct ActiveTranslation {
    pub(crate) id: u64,
    pub(crate) cancellation: CancellationToken,
}

pub(crate) struct TranslationState {
    pub(crate) controls: TranslationControls,
    pub(crate) source: String,
    pub(crate) source_language: String,
    pub(crate) target_language: String,
    pub(crate) system_prompt: String,
    pub(crate) user_prompt: String,
    pub(crate) model_id: Option<String>,
    pub(crate) reasoning_preset: Option<String>,
    pub(crate) response: Option<AssistantResponse>,
    pub(crate) request: Option<RequestInfo>,
    pub(crate) active: Option<ActiveTranslation>,
    pub(crate) next_operation_id: u64,
    pub(crate) error: Option<String>,
    pub(crate) result_scroll: ScrollHandle,
}

impl TranslationState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<OneChat>) -> Self {
        Self {
            controls: TranslationControls::new(window, cx),
            source: String::new(),
            source_language: "Auto Detect".into(),
            target_language: "English".into(),
            system_prompt: DEFAULT_TRANSLATION_SYSTEM_PROMPT.into(),
            user_prompt: DEFAULT_TRANSLATION_USER_PROMPT.into(),
            model_id: None,
            reasoning_preset: None,
            response: None,
            request: None,
            active: None,
            next_operation_id: 0,
            error: None,
            result_scroll: ScrollHandle::new(),
        }
    }

    pub(crate) fn is_generating(&self) -> bool {
        self.active.is_some()
    }

    pub(crate) fn uses_default_prompts(&self, system_prompt: &str, user_prompt: &str) -> bool {
        self.system_prompt == system_prompt && self.user_prompt == user_prompt
    }
}
