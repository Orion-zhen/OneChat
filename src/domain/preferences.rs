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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct AppSettings {
    pub current_conversation_id: Option<String>,
    pub primary_model_id: Option<String>,
    pub sidebar_collapsed: bool,
    pub theme: Theme,
    pub default_system_prompt: String,
    pub message_width_ratio: f32,
}

impl AppSettings {
    pub fn message_width_ratio(&self) -> f32 {
        self.message_width_ratio
            .clamp(MIN_MESSAGE_WIDTH_RATIO, MAX_MESSAGE_WIDTH_RATIO)
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            current_conversation_id: None,
            primary_model_id: None,
            sidebar_collapsed: false,
            theme: Theme::default(),
            default_system_prompt: String::new(),
            message_width_ratio: DEFAULT_MESSAGE_WIDTH_RATIO,
        }
    }
}
