pub mod openai;

use async_channel::Sender;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

pub use crate::model::TokenUsage;
use crate::model::{GenerationConfig, MessageRole, Model, Provider, ProviderKind};

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
    Failed(AppError),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorKind {
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

impl AppErrorKind {
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
pub struct AppError {
    pub kind: AppErrorKind,
    pub message: String,
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(kind: AppErrorKind, message: impl Into<String>) -> Self {
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
        Self::new(AppErrorKind::Network, "Network request failed").with_detail(error.to_string())
    }

    pub fn cancelled() -> Self {
        Self::new(AppErrorKind::UserCancelled, "Generation stopped")
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AppError {}

pub async fn test_connection(provider: &Provider) -> Result<(), AppError> {
    match provider.kind {
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
            openai::test_connection(provider).await
        }
        ProviderKind::Anthropic | ProviderKind::Gemini => Err(AppError::new(
            AppErrorKind::UnsupportedParameter,
            "Connection testing is not available for this provider yet",
        )),
    }
}

pub async fn generate(
    request: GenerationRequest,
    events: Sender<GenerationEvent>,
    cancellation: CancellationToken,
) {
    let result = match request.provider.kind {
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
            openai::stream(request, &events, cancellation).await
        }
        ProviderKind::Anthropic | ProviderKind::Gemini => Err(AppError::new(
            AppErrorKind::UnsupportedParameter,
            "Streaming is not available for this provider yet",
        )),
    };
    if let Err(error) = result {
        let _ = events.send(GenerationEvent::Failed(error)).await;
    }
}
