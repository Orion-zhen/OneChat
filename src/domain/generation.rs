use serde::{Deserialize, Serialize};

use super::{GenerationConfig, MessageRole, Model, Provider, Timestamp, new_id, now_timestamp};

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct GenerationRequest {
    pub provider: Provider,
    pub model: Model,
    pub system_prompt: String,
    pub config: GenerationConfig,
    pub messages: Vec<ChatMessage>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GenerationEvent {
    Started,
    TextDelta(String),
    ThinkingDelta(String),
    UsageUpdated(TokenUsage),
    Completed,
    Failed(GenerationError),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationErrorKind {
    Authentication,
    ProviderUnavailable,
    ModelNotFound,
    RateLimited,
    ContextLengthExceeded,
    UnsupportedParameter,
    Network,
    StreamInterrupted,
    UserCancelled,
    Unknown,
}

impl GenerationErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Authentication => "authentication",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::ModelNotFound => "model_not_found",
            Self::RateLimited => "rate_limited",
            Self::ContextLengthExceeded => "context_length_exceeded",
            Self::UnsupportedParameter => "unsupported_parameter",
            Self::Network => "network",
            Self::StreamInterrupted => "stream_interrupted",
            Self::UserCancelled => "user_cancelled",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GenerationError {
    pub kind: GenerationErrorKind,
    pub message: String,
    pub detail: Option<String>,
}

impl GenerationError {
    pub fn new(kind: GenerationErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn network(error: impl std::fmt::Display) -> Self {
        Self::new(GenerationErrorKind::Network, "Network request failed")
            .with_detail(error.to_string())
    }

    pub fn cancelled() -> Self {
        Self::new(GenerationErrorKind::UserCancelled, "Generation stopped")
    }
}

impl std::fmt::Display for GenerationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GenerationError {}

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
    pub ttft_ms: Option<u64>,
    pub duration_ms: Option<u64>,
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
            ttft_ms: None,
            duration_ms: None,
        }
    }
}
