pub use rig_core::completion::{Message, ToolDefinition};
use rig_core::{
    completion::AssistantContent,
    message::{Reasoning, ToolCall},
};
use serde::{Deserialize, Serialize};

use super::{GenerationConfig, HistoryLimit, Model, Provider, Timestamp, new_id, now_timestamp};

#[derive(Clone, Debug)]
pub struct GenerationRequest {
    pub provider: Provider,
    pub model: Model,
    pub system_prompt: String,
    pub config: GenerationConfig,
    pub messages: Vec<Message>,
    pub audio_duration_ms: u64,
    pub tools: Vec<ToolDefinition>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GenerationEvent {
    Started,
    TextDelta(String),
    ThinkingDelta {
        provider_id: Option<String>,
        delta: String,
    },
    ToolCallObserved {
        stream_call_id: String,
        call_id: Option<String>,
    },
    StepStarted {
        estimated_input_tokens: u64,
    },
    UsageUpdated(TokenUsage),
    ToolExecutionUpdated(Box<ToolExecution>),
    TranscriptAppended(Box<Message>),
    TranscriptContinued(Box<Message>),
    Completed,
    Failed(GenerationError),
}

pub fn continue_last_assistant(messages: &mut Vec<Message>, continuation: Message) {
    let Some(Message::Assistant {
        content: existing, ..
    }) = messages.last_mut()
    else {
        messages.push(continuation);
        return;
    };
    let Message::Assistant { content, .. } = continuation else {
        messages.push(continuation);
        return;
    };

    let mut continuation = strip_replayed_assistant_prefix(existing, content).into_iter();
    if let Some(first) = continuation.next() {
        if let (Some(AssistantContent::Text(existing)), AssistantContent::Text(continued)) =
            (existing.iter_mut().last(), &first)
        {
            existing.text.push_str(&continued.text);
        } else {
            existing.push(first);
        }
    }
    for item in continuation {
        existing.push(item);
    }
}

fn strip_replayed_assistant_prefix(
    existing: &[AssistantContent],
    continuation: Vec<AssistantContent>,
) -> Vec<AssistantContent> {
    let (existing_text, existing_reasoning) = assistant_channels(existing.iter());
    let (continued_text, continued_reasoning) = assistant_channels(continuation.iter());
    let mut text_prefix = replayed_prefix_len(&existing_text, &continued_text);
    let mut reasoning_prefix = replayed_prefix_len(&existing_reasoning, &continued_reasoning);
    let mut normalized = Vec::with_capacity(continuation.len());

    for item in continuation {
        match item {
            AssistantContent::Text(mut text) => {
                if strip_prefix(&mut text.text, &mut text_prefix) {
                    normalized.push(AssistantContent::Text(text));
                }
            }
            AssistantContent::Reasoning(reasoning) if reasoning_prefix > 0 => {
                let id = reasoning.id.clone();
                let mut content = reasoning.display_text();
                if strip_prefix(&mut content, &mut reasoning_prefix) {
                    let mut reasoning = Reasoning::new(&content);
                    reasoning.id = id;
                    normalized.push(AssistantContent::Reasoning(reasoning));
                }
            }
            item => normalized.push(item),
        }
    }
    normalized
}

fn assistant_channels<'a>(
    content: impl IntoIterator<Item = &'a AssistantContent>,
) -> (String, String) {
    let mut text = String::new();
    let mut reasoning = String::new();
    for item in content {
        match item {
            AssistantContent::Text(item) => text.push_str(&item.text),
            AssistantContent::Reasoning(item) => reasoning.push_str(&item.display_text()),
            _ => {}
        }
    }
    (text, reasoning)
}

fn replayed_prefix_len(existing: &str, continuation: &str) -> usize {
    if !existing.is_empty() && continuation.starts_with(existing) {
        existing.len()
    } else {
        0
    }
}

fn strip_prefix(content: &mut String, remaining: &mut usize) -> bool {
    if *remaining == 0 {
        return !content.is_empty();
    }
    if *remaining >= content.len() {
        *remaining -= content.len();
        return false;
    }
    *content = content.split_off(*remaining);
    *remaining = 0;
    true
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
    pub call_id: String,
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
        call_id: impl Into<String>,
        server_id: impl Into<String>,
        tool_name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self {
            id: new_id("tool"),
            call_id: call_id.into(),
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
pub enum RequestKind {
    #[default]
    Generate,
    Additional,
    Regenerate,
    Continue,
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequestContextInfo {
    pub history_limit: HistoryLimit,
    pub available_history_turns: u32,
    pub included_history_turns: u32,
    pub limited_by_context_window: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RequestInfo {
    pub id: String,
    #[serde(default)]
    pub kind: RequestKind,
    pub conversation_id: String,
    pub turn_id: String,
    pub response_id: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub status: RequestStatus,
    pub usage: TokenUsage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_step_input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_step_estimated_input_tokens: Option<u64>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<super::PromptSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_opening: Option<super::PromptSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<RequestContextInfo>,
}

impl RequestInfo {
    pub fn new(
        conversation_id: impl Into<String>,
        turn_id: impl Into<String>,
        response_id: impl Into<String>,
    ) -> Self {
        Self {
            id: new_id("request"),
            kind: RequestKind::Generate,
            conversation_id: conversation_id.into(),
            turn_id: turn_id.into(),
            response_id: response_id.into(),
            provider_id: None,
            model_id: None,
            status: RequestStatus::Sending,
            usage: TokenUsage::default(),
            last_step_input_tokens: None,
            last_step_estimated_input_tokens: None,
            error: None,
            started_at: now_timestamp(),
            first_token_at: None,
            finished_at: None,
            ttft_ms: None,
            thinking_duration_ms: None,
            tool_call_count: 0,
            tool_duration_ms: None,
            duration_ms: None,
            system_prompt: None,
            assistant_opening: None,
            context: None,
        }
    }
}
