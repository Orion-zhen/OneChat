use gpui::{Context, Window};

use super::{controls::TranslationControls, output::TranslationOutput};
use crate::{
    desktop::app::OneChat,
    domain::{DEFAULT_TRANSLATION_SYSTEM_PROMPT, DEFAULT_TRANSLATION_USER_PROMPT},
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

pub(crate) struct TranslationState {
    pub(crate) controls: TranslationControls,
    pub(crate) output: TranslationOutput,
    pub(crate) source: String,
    pub(crate) source_language: String,
    pub(crate) target_language: String,
    pub(crate) system_prompt: String,
    pub(super) prompts_dirty: bool,
    pub(crate) user_prompt: String,
    pub(crate) model_id: Option<String>,
    pub(crate) reasoning_preset: Option<String>,
    pub(crate) error: Option<String>,
}

impl TranslationState {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<OneChat>) -> Self {
        Self {
            controls: TranslationControls::new(window, cx),
            output: Default::default(),
            source: String::new(),
            source_language: "Auto Detect".into(),
            target_language: "English".into(),
            system_prompt: DEFAULT_TRANSLATION_SYSTEM_PROMPT.into(),
            prompts_dirty: false,
            user_prompt: DEFAULT_TRANSLATION_USER_PROMPT.into(),
            model_id: None,
            reasoning_preset: None,
            error: None,
        }
    }

    pub(crate) fn is_generating(&self) -> bool {
        self.output.is_generating()
    }

    pub(crate) fn uses_default_prompts(&self, system_prompt: &str, user_prompt: &str) -> bool {
        self.system_prompt == system_prompt && self.user_prompt == user_prompt
    }
}
