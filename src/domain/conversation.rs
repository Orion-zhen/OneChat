use serde::{Deserialize, Serialize};

use super::{GenerationConfig, Model, Timestamp, new_id, now_timestamp};

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
    pub fn new(
        title: impl Into<String>,
        model: Option<&Model>,
        default_system_prompt: &str,
    ) -> Self {
        let now = now_timestamp();
        let prompt = default_system_prompt.trim().to_string();
        Self {
            id: new_id("conversation"),
            title: title.into(),
            model_id: model.map(|model| model.id.clone()),
            system_prompt: SystemPrompt {
                source: if prompt.is_empty() {
                    SystemPromptSource::None
                } else {
                    SystemPromptSource::FromDefault
                },
                content: prompt,
            },
            generation_config: GenerationConfig::default(),
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
