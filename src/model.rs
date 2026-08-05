use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

static ID_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub type Timestamp = i64;

pub fn now_timestamp() -> Timestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as Timestamp
}

pub fn new_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{nanos:x}-{sequence:x}")
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[default]
    OpenAi,
    OpenAiCompatible,
    Anthropic,
    Gemini,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "open_ai",
            Self::OpenAiCompatible => "open_ai_compatible",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::OpenAiCompatible => "OpenAI-compatible",
            Self::Anthropic => "Anthropic",
            Self::Gemini => "Gemini",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub endpoint: String,
    pub api_key: String,
    pub headers: BTreeMap<String, String>,
    pub proxy: Option<String>,
    pub enabled: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Provider {
    pub fn new(name: impl Into<String>, kind: ProviderKind) -> Self {
        let now = now_timestamp();
        Self {
            id: new_id("provider"),
            name: name.into(),
            kind,
            endpoint: String::new(),
            api_key: String::new(),
            headers: BTreeMap::new(),
            proxy: None,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ModelCapabilities {
    pub streaming: bool,
    pub system_prompt: bool,
    pub vision: bool,
    pub thinking: bool,
    pub temperature: bool,
    pub top_p: bool,
    pub top_k: bool,
    pub max_output_tokens: bool,
    pub frequency_penalty: bool,
    pub presence_penalty: bool,
    pub seed: bool,
    pub stop_sequences: bool,
    pub thinking_budget: bool,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            streaming: true,
            system_prompt: true,
            vision: false,
            thinking: false,
            temperature: true,
            top_p: true,
            top_k: false,
            max_output_tokens: true,
            frequency_penalty: false,
            presence_penalty: false,
            seed: false,
            stop_sequences: true,
            thinking_budget: false,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct GenerationConfig {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub seed: Option<i64>,
    pub stop_sequences: Vec<String>,
    pub thinking_budget: Option<i64>,
    #[serde(default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Model {
    pub id: String,
    pub provider_id: String,
    pub remote_id: String,
    pub display_name: String,
    pub capabilities: ModelCapabilities,
    pub default_config: GenerationConfig,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Model {
    pub fn new(
        provider_id: impl Into<String>,
        remote_id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        let now = now_timestamp();
        Self {
            id: new_id("model"),
            provider_id: provider_id.into(),
            remote_id: remote_id.into(),
            display_name: display_name.into(),
            capabilities: ModelCapabilities::default(),
            default_config: GenerationConfig::default(),
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemPromptSource {
    #[default]
    None,
    FromDefault,
    Custom,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SystemPrompt {
    pub content: String,
    pub source: SystemPromptSource,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub model_id: Option<String>,
    pub system_prompt: SystemPrompt,
    pub generation_config: GenerationConfig,
    pub pinned: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Conversation {
    pub fn new(title: impl Into<String>, model: Option<&Model>) -> Self {
        let now = now_timestamp();
        Self {
            id: new_id("conversation"),
            title: title.into(),
            model_id: model.map(|model| model.id.clone()),
            system_prompt: SystemPrompt::default(),
            generation_config: model
                .map(|model| model.default_config.clone())
                .unwrap_or_default(),
            pinned: false,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
}

impl MessageRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Pending,
    Streaming,
    #[default]
    Completed,
    Stopped,
    Failed,
    Interrupted,
}

impl MessageStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Streaming => "streaming",
            Self::Completed => "completed",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub request_id: Option<String>,
    pub role: MessageRole,
    pub status: MessageStatus,
    pub content: String,
    pub thinking: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Message {
    pub fn new(
        conversation_id: impl Into<String>,
        role: MessageRole,
        content: impl Into<String>,
    ) -> Self {
        let now = now_timestamp();
        Self {
            id: new_id("message"),
            conversation_id: conversation_id.into(),
            request_id: None,
            role,
            status: MessageStatus::Completed,
            content: content.into(),
            thinking: String::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestStatus {
    Sending,
    Streaming,
    Stopped,
    Failed,
    #[default]
    Completed,
    Interrupted,
}

impl RequestStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sending => "sending",
            Self::Streaming => "streaming",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
            Self::Completed => "completed",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub estimated: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequestError {
    pub kind: String,
    pub message: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RequestInfo {
    pub id: String,
    pub conversation_id: String,
    pub assistant_message_id: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub status: RequestStatus,
    pub usage: TokenUsage,
    pub error: Option<RequestError>,
    pub started_at: Timestamp,
    pub first_token_at: Option<Timestamp>,
    pub finished_at: Option<Timestamp>,
}

impl RequestInfo {
    pub fn new(
        conversation_id: impl Into<String>,
        assistant_message_id: impl Into<String>,
    ) -> Self {
        Self {
            id: new_id("request"),
            conversation_id: conversation_id.into(),
            assistant_message_id: assistant_message_id.into(),
            provider_id: None,
            model_id: None,
            status: RequestStatus::Sending,
            usage: TokenUsage::default(),
            error: None,
            started_at: now_timestamp(),
            first_token_at: None,
            finished_at: None,
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

impl Theme {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct AppSettings {
    pub current_conversation_id: Option<String>,
    pub sidebar_collapsed: bool,
    pub theme: Theme,
    pub reduce_motion: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Page {
    #[default]
    Chat,
    Settings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationGroup {
    Pinned,
    Today,
    Yesterday,
    PreviousSevenDays,
    Older,
}

impl ConversationGroup {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pinned => "Pinned",
            Self::Today => "Today",
            Self::Yesterday => "Yesterday",
            Self::PreviousSevenDays => "Previous 7 days",
            Self::Older => "Older",
        }
    }

    pub fn for_conversation(conversation: &Conversation, now: Timestamp) -> Self {
        if conversation.pinned {
            return Self::Pinned;
        }
        let Some(now) = DateTime::from_timestamp(now, 0) else {
            return Self::Older;
        };
        let Some(updated_at) = DateTime::from_timestamp(conversation.updated_at, 0) else {
            return Self::Older;
        };
        let days = now
            .with_timezone(&Local)
            .date_naive()
            .signed_duration_since(updated_at.with_timezone(&Local).date_naive())
            .num_days();
        match days {
            ..=0 => Self::Today,
            1 => Self::Yesterday,
            2..=6 => Self::PreviousSevenDays,
            _ => Self::Older,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_config_has_no_preset() {
        let json = serde_json::to_value(GenerationConfig::default()).unwrap();
        assert!(json.get("preset").is_none());
    }

    #[test]
    fn pinned_conversations_have_their_own_group() {
        let mut conversation = Conversation::new("Pinned", None);
        conversation.pinned = true;
        assert_eq!(
            ConversationGroup::for_conversation(&conversation, now_timestamp()),
            ConversationGroup::Pinned
        );
    }

    #[test]
    fn conversations_are_grouped_by_local_calendar_date() {
        let now = 1_705_320_000;
        let mut conversation = Conversation::new("Yesterday", None);
        conversation.updated_at = now - 24 * 60 * 60;

        assert_eq!(
            ConversationGroup::for_conversation(&conversation, now),
            ConversationGroup::Yesterday
        );
    }
}
