use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::PromptVariableSource;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SendMessageShortcut {
    Enter,
    #[default]
    SecondaryEnter,
}

pub const DEFAULT_MESSAGE_WIDTH_RATIO: f32 = 0.7;
pub const MIN_MESSAGE_WIDTH_RATIO: f32 = 0.5;
pub const MAX_MESSAGE_WIDTH_RATIO: f32 = 1.0;
pub const DEFAULT_MESSAGE_FONT_SIZE: f32 = 16.0;
pub const MIN_MESSAGE_FONT_SIZE: f32 = 13.0;
pub const MAX_MESSAGE_FONT_SIZE: f32 = 22.0;
pub const DEFAULT_BACKGROUND_OPACITY: f32 = 0.8;
pub const DEFAULT_THEME_COLOR: &str = "#007AFF";
pub const MIN_BACKGROUND_OPACITY: f32 = 0.0;
pub const MAX_BACKGROUND_OPACITY: f32 = 1.0;
pub const DEFAULT_UI_FONT_FAMILY: &str = ".SystemUIFont";
pub const DEFAULT_CODE_FONT_FAMILY: &str = if cfg!(target_os = "macos") {
    "SFMono-Regular"
} else if cfg!(target_os = "windows") {
    "Consolas"
} else {
    "DejaVu Sans Mono"
};
pub const DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT: &str = "Create a concise title in the user's language, no longer than 15 characters or 5 short words. Return only the plain title without quotes, Markdown, labels, or explanation.";

pub fn normalize_font_families(families: Vec<String>, default: &str) -> Vec<String> {
    let mut normalized = Vec::new();
    for family in families {
        let family = family.trim();
        if !family.is_empty() && !normalized.iter().any(|item| item == family) {
            normalized.push(family.to_string());
        }
    }
    if normalized.is_empty() {
        normalized.push(default.to_string());
    }
    normalized
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct AppSettings {
    pub current_conversation_id: Option<String>,
    pub primary_model_id: Option<String>,
    pub title_generation_model_id: Option<String>,
    pub title_generation_reasoning_preset: Option<String>,
    pub auto_title_enabled: bool,
    pub send_message_shortcut: SendMessageShortcut,
    pub sidebar_collapsed: bool,
    pub theme: Theme,
    pub theme_color: String,
    pub ui_font_families: Vec<String>,
    pub code_font_families: Vec<String>,
    pub code_block_wrap: bool,
    pub parse_document_images: bool,
    pub default_system_prompt_preset: Option<String>,
    pub title_generation_system_prompt: String,
    pub prompt_variables: BTreeMap<String, PromptVariableSource>,
    pub message_font_size: f32,
    pub message_width_ratio: f32,
    pub background_opacity: f32,
}

impl AppSettings {
    pub fn normalize_fonts(&mut self) -> bool {
        let ui = normalize_font_families(self.ui_font_families.clone(), DEFAULT_UI_FONT_FAMILY);
        let code =
            normalize_font_families(self.code_font_families.clone(), DEFAULT_CODE_FONT_FAMILY);
        let changed = self.ui_font_families != ui || self.code_font_families != code;
        self.ui_font_families = ui;
        self.code_font_families = code;
        changed
    }

    pub fn message_width_ratio(&self) -> f32 {
        self.message_width_ratio
            .clamp(MIN_MESSAGE_WIDTH_RATIO, MAX_MESSAGE_WIDTH_RATIO)
    }

    pub fn message_font_size(&self) -> f32 {
        self.message_font_size
            .clamp(MIN_MESSAGE_FONT_SIZE, MAX_MESSAGE_FONT_SIZE)
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
            title_generation_reasoning_preset: None,
            auto_title_enabled: true,
            send_message_shortcut: SendMessageShortcut::default(),
            sidebar_collapsed: false,
            theme: Theme::default(),
            theme_color: DEFAULT_THEME_COLOR.into(),
            ui_font_families: vec![DEFAULT_UI_FONT_FAMILY.into()],
            code_font_families: vec![DEFAULT_CODE_FONT_FAMILY.into()],
            code_block_wrap: false,
            parse_document_images: true,
            default_system_prompt_preset: None,
            title_generation_system_prompt: DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT.into(),
            prompt_variables: BTreeMap::new(),
            message_font_size: DEFAULT_MESSAGE_FONT_SIZE,
            message_width_ratio: DEFAULT_MESSAGE_WIDTH_RATIO,
            background_opacity: DEFAULT_BACKGROUND_OPACITY,
        }
    }
}
