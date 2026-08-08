use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

pub const DEFAULT_MESSAGE_WIDTH_RATIO: f32 = 0.7;
pub const MIN_MESSAGE_WIDTH_RATIO: f32 = 0.5;
pub const MAX_MESSAGE_WIDTH_RATIO: f32 = 1.0;
pub const DEFAULT_BACKGROUND_OPACITY: f32 = 0.5;
pub const MIN_BACKGROUND_OPACITY: f32 = 0.0;
pub const MAX_BACKGROUND_OPACITY: f32 = 1.0;
pub const DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT: &str = "Generate a concise title for this conversation. Use the same language as the user. Return only the title without quotes, Markdown, labels, or explanation.";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct AppSettings {
    pub current_conversation_id: Option<String>,
    pub primary_model_id: Option<String>,
    pub title_generation_model_id: Option<String>,
    pub auto_title_enabled: bool,
    pub sidebar_collapsed: bool,
    pub theme: Theme,
    pub default_system_prompt_preset: Option<String>,
    pub title_generation_system_prompt: String,
    pub message_width_ratio: f32,
    pub background_opacity: f32,
}

impl AppSettings {
    pub fn message_width_ratio(&self) -> f32 {
        self.message_width_ratio
            .clamp(MIN_MESSAGE_WIDTH_RATIO, MAX_MESSAGE_WIDTH_RATIO)
    }

    pub fn background_opacity(&self) -> f32 {
        self.background_opacity
            .clamp(MIN_BACKGROUND_OPACITY, MAX_BACKGROUND_OPACITY)
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            current_conversation_id: None,
            primary_model_id: None,
            title_generation_model_id: None,
            auto_title_enabled: true,
            sidebar_collapsed: false,
            theme: Theme::default(),
            default_system_prompt_preset: None,
            title_generation_system_prompt: DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT.into(),
            message_width_ratio: DEFAULT_MESSAGE_WIDTH_RATIO,
            background_opacity: DEFAULT_BACKGROUND_OPACITY,
        }
    }
}
