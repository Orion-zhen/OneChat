use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::PromptVariableSource;

pub const MAX_LIMITED_HISTORY_TURNS: u32 = 50;
pub const HISTORY_LIMIT_SLIDER_MIN: f32 = 0.0;
pub const HISTORY_LIMIT_SLIDER_MAX: f32 = MAX_LIMITED_HISTORY_TURNS as f32 + 1.0;
pub const HISTORY_LIMIT_SLIDER_STEP: f32 = 1.0;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", content = "turns", rename_all = "snake_case")]
pub enum HistoryLimit {
    #[default]
    Unlimited,
    Last(u32),
}

impl HistoryLimit {
    pub fn normalized(self) -> Self {
        match self {
            Self::Unlimited => Self::Unlimited,
            Self::Last(turns) => Self::Last(turns.min(MAX_LIMITED_HISTORY_TURNS)),
        }
    }

    pub fn from_slider_value(value: f32) -> Self {
        let position = value
            .round()
            .clamp(HISTORY_LIMIT_SLIDER_MIN, HISTORY_LIMIT_SLIDER_MAX)
            as u32;
        if position > MAX_LIMITED_HISTORY_TURNS {
            Self::Unlimited
        } else {
            Self::Last(position)
        }
    }

    pub fn slider_value(self) -> f32 {
        match self.normalized() {
            Self::Unlimited => HISTORY_LIMIT_SLIDER_MAX,
            Self::Last(turns) => turns as f32,
        }
    }

    pub fn label(self) -> String {
        match self.normalized() {
            Self::Unlimited => "Unlimited".into(),
            Self::Last(turns) => format!("{turns} turns"),
        }
    }
}

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
    pub history_limit: HistoryLimit,
    pub default_system_prompt_preset: Option<String>,
    pub title_generation_system_prompt: String,
    pub prompt_variables: BTreeMap<String, PromptVariableSource>,
    pub message_font_size: f32,
    pub message_width_ratio: f32,
    pub background_opacity: f32,
}

impl AppSettings {
    pub fn normalize(&mut self) -> bool {
        let fonts_changed = self.normalize_fonts();
        let history_limit = self.history_limit.normalized();
        let history_changed = self.history_limit != history_limit;
        self.history_limit = history_limit;
        fonts_changed || history_changed
    }

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
            history_limit: HistoryLimit::default(),
            default_system_prompt_preset: None,
            title_generation_system_prompt: DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT.into(),
            prompt_variables: BTreeMap::new(),
            message_font_size: DEFAULT_MESSAGE_FONT_SIZE,
            message_width_ratio: DEFAULT_MESSAGE_WIDTH_RATIO,
            background_opacity: DEFAULT_BACKGROUND_OPACITY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_limit_normalizes_and_maps_slider_values() {
        assert_eq!(HistoryLimit::Last(51).normalized(), HistoryLimit::Last(50));
        assert_eq!(HistoryLimit::from_slider_value(-1.0), HistoryLimit::Last(0));
        assert_eq!(HistoryLimit::from_slider_value(1.49), HistoryLimit::Last(1));
        assert_eq!(HistoryLimit::from_slider_value(1.5), HistoryLimit::Last(2));
        assert_eq!(
            HistoryLimit::from_slider_value(50.0),
            HistoryLimit::Last(50)
        );
        assert_eq!(
            HistoryLimit::from_slider_value(51.0),
            HistoryLimit::Unlimited
        );
        assert_eq!(
            HistoryLimit::from_slider_value(100.0),
            HistoryLimit::Unlimited
        );
        assert_eq!(HistoryLimit::Last(0).slider_value(), 0.0);
        assert_eq!(HistoryLimit::Last(1).slider_value(), 1.0);
        assert_eq!(HistoryLimit::Last(30).slider_value(), 30.0);
        assert_eq!(HistoryLimit::Last(50).slider_value(), 50.0);
        assert_eq!(HistoryLimit::Unlimited.slider_value(), 51.0);
        assert_eq!(HistoryLimit::Last(0).label(), "0 turns");
        assert_eq!(HistoryLimit::Last(1).label(), "1 turns");
        assert_eq!(HistoryLimit::Last(50).label(), "50 turns");
        assert_eq!(HistoryLimit::Unlimited.label(), "Unlimited");
    }

    #[test]
    fn app_settings_normalizes_history_limit() {
        let mut settings = AppSettings {
            history_limit: HistoryLimit::Last(100),
            ..AppSettings::default()
        };

        assert!(settings.normalize());
        assert_eq!(settings.history_limit, HistoryLimit::Last(50));
        assert!(!settings.normalize());
    }
}
