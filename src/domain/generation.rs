pub use rig_core::completion::{Message, ToolDefinition};
use rig_core::{completion::AssistantContent, message::ToolCall};
use serde::{Deserialize, Serialize};

use super::{GenerationConfig, Model, Provider, Timestamp, new_id, now_timestamp};

#[derive(Clone, Debug)]
pub struct GenerationRequest {
    pub provider: Provider,
    pub model: Model,
    pub system_prompt: String,
    pub config: GenerationConfig,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GenerationEvent {
    Started,
    TextDelta(String),
    ThinkingDelta(String),
    UsageUpdated(TokenUsage),
    ProviderOutput,
    ToolExecutionUpdated(Box<ToolExecution>),
    TranscriptAppended(Box<Message>),
    Completed,
    Failed(GenerationError),
}

pub fn message_tool_calls(message: &Message) -> Vec<ToolCall> {
    let Message::Assistant { content, .. } = message else {
        return Vec::new();
    };
    content
        .iter()
        .filter_map(|content| match content {
            AssistantContent::ToolCall(call) => Some(call.clone()),
            _ => None,
        })
        .collect()
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
pub enum ToolExecutionStatus {
    #[default]
    Queued,
    Running,
    Completed,
    Failed,
    Stopped,
    Interrupted,
}

impl ToolExecutionStatus {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ToolExecution {
    pub id: String,
    pub provider_tool_call_id: String,
    pub provider_call_id: Option<String>,
    pub server_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub status: ToolExecutionStatus,
    pub result: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<Timestamp>,
    pub finished_at: Option<Timestamp>,
    pub duration_ms: Option<u64>,
}

impl ToolExecution {
    pub fn new(
        provider_tool_call_id: impl Into<String>,
        provider_call_id: Option<String>,
        server_id: impl Into<String>,
        tool_name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self {
            id: new_id("tool"),
            provider_tool_call_id: provider_tool_call_id.into(),
            provider_call_id,
            server_id: server_id.into(),
            tool_name: tool_name.into(),
            arguments,
            status: ToolExecutionStatus::Queued,
            result: None,
            error: None,
            started_at: None,
            finished_at: None,
            duration_ms: None,
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
    pub turn_id: String,
    pub response_id: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub status: RequestStatus,
    pub usage: TokenUsage,
    pub error: Option<RequestError>,
    pub started_at: Timestamp,
    pub first_token_at: Option<Timestamp>,
    pub finished_at: Option<Timestamp>,
    pub ttft_ms: Option<u64>,
    pub thinking_duration_ms: Option<u64>,
    #[serde(default)]
    pub tool_call_count: u64,
    #[serde(default)]
    pub tool_duration_ms: Option<u64>,
    pub duration_ms: Option<u64>,
}

impl RequestInfo {
    pub fn new(
        conversation_id: impl Into<String>,
        turn_id: impl Into<String>,
        response_id: impl Into<String>,
    ) -> Self {
        Self {
            id: new_id("request"),
            conversation_id: conversation_id.into(),
            turn_id: turn_id.into(),
            response_id: response_id.into(),
            provider_id: None,
            model_id: None,
            status: RequestStatus::Sending,
            usage: TokenUsage::default(),
            error: None,
            started_at: now_timestamp(),
            first_token_at: None,
            finished_at: None,
            ttft_ms: None,
            thinking_duration_ms: None,
            tool_call_count: 0,
            tool_duration_ms: None,
            duration_ms: None,
        }
    }
}
