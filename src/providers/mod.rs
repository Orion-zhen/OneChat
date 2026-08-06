pub mod anthropic;
pub mod gemini;
pub mod openai;

use async_channel::Sender;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use rig_core::{
    OneOrMany,
    completion::{AssistantContent, CompletionError, CompletionRequest, Message},
    message::UserContent,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
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
        ProviderKind::Anthropic => anthropic::test_connection(provider).await,
        ProviderKind::Gemini => gemini::test_connection(provider).await,
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
        ProviderKind::Anthropic => anthropic::stream(request, &events, cancellation).await,
        ProviderKind::Gemini => gemini::stream(request, &events, cancellation).await,
    };
    if let Err(error) = result {
        let _ = events.send(GenerationEvent::Failed(error)).await;
    }
}

pub(crate) fn sdk_http_client(provider: &Provider) -> Result<reqwest::Client, AppError> {
    let mut builder = reqwest::Client::builder().no_proxy();
    if let Some(proxy) = provider
        .proxy
        .as_deref()
        .filter(|proxy| !proxy.trim().is_empty())
    {
        builder = builder.proxy(reqwest::Proxy::all(proxy).map_err(|error| {
            AppError::new(AppErrorKind::UnsupportedParameter, "Invalid proxy URL")
                .with_detail(error.to_string())
        })?);
    }
    builder.build().map_err(AppError::network)
}

pub(crate) fn sdk_headers(provider: &Provider) -> Result<HeaderMap, AppError> {
    provider
        .headers
        .iter()
        .map(|(name, value)| {
            let name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                AppError::new(
                    AppErrorKind::UnsupportedParameter,
                    "Invalid custom header name",
                )
                .with_detail(error.to_string())
            })?;
            let value = HeaderValue::from_str(value).map_err(|error| {
                AppError::new(
                    AppErrorKind::UnsupportedParameter,
                    "Invalid custom header value",
                )
                .with_detail(error.to_string())
            })?;
            Ok((name, value))
        })
        .collect()
}

pub(crate) fn sdk_base_url(provider: &Provider) -> Result<String, AppError> {
    let base = if provider.endpoint.trim().is_empty() {
        provider.kind.default_endpoint()
    } else {
        provider.endpoint.trim()
    };
    if base.is_empty() {
        return Err(AppError::new(
            AppErrorKind::UnsupportedParameter,
            "Provider endpoint is required",
        ));
    }

    let mut base = base.trim_end_matches('/');
    match provider.kind {
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
            for suffix in ["/chat/completions", "/models"] {
                if let Some(root) = base.strip_suffix(suffix) {
                    base = root;
                    break;
                }
            }
        }
        ProviderKind::Anthropic => {
            for suffix in ["/v1/messages", "/v1/models", "/messages", "/models", "/v1"] {
                if let Some(root) = base.strip_suffix(suffix) {
                    base = root;
                    break;
                }
            }
        }
        ProviderKind::Gemini => {
            if let Some(index) = base.rfind("/v1beta") {
                base = &base[..index];
            }
        }
    }
    Ok(base.to_string())
}

pub(crate) fn sdk_request(
    request: &GenerationRequest,
    additional_params: Map<String, Value>,
) -> Result<CompletionRequest, AppError> {
    let messages = request
        .messages
        .iter()
        .map(|message| match message.role {
            MessageRole::User => Message::User {
                content: OneOrMany::one(UserContent::text(message.content.clone())),
            },
            MessageRole::Assistant => Message::Assistant {
                id: None,
                content: OneOrMany::one(AssistantContent::text(message.content.clone())),
            },
        })
        .collect::<Vec<_>>();
    let chat_history = OneOrMany::many(messages).map_err(|_| {
        AppError::new(
            AppErrorKind::UnsupportedParameter,
            "At least one chat message is required",
        )
    })?;
    let capabilities = &request.model.capabilities;

    Ok(CompletionRequest {
        model: Some(request.model.remote_id.clone()),
        preamble: (!request.system_prompt.trim().is_empty()).then(|| request.system_prompt.clone()),
        chat_history,
        documents: Vec::new(),
        tools: Vec::new(),
        temperature: capabilities
            .temperature
            .then_some(request.config.temperature)
            .flatten(),
        max_tokens: capabilities
            .max_output_tokens
            .then_some(request.config.max_output_tokens)
            .flatten()
            .map(u64::from),
        tool_choice: None,
        additional_params: (!additional_params.is_empty())
            .then_some(Value::Object(additional_params)),
        output_schema: None,
        record_telemetry_content: false,
    })
}

pub(crate) fn sdk_verify_error(error: rig_core::client::VerifyError) -> AppError {
    if let Some(status) = error.provider_response_status() {
        return classify_provider_error(
            status,
            error.provider_response_body().unwrap_or_default(),
            Some(error.to_string()),
        );
    }

    match error {
        rig_core::client::VerifyError::InvalidAuthentication => {
            AppError::new(AppErrorKind::Authentication, "Authentication failed")
        }
        rig_core::client::VerifyError::HttpError(_) => AppError::network(error),
        _ => AppError::new(AppErrorKind::Unknown, "Provider connection test failed")
            .with_detail(error.to_string()),
    }
}

pub(crate) fn sdk_completion_error(error: CompletionError, had_output: bool) -> AppError {
    if let Some(status) = error.provider_response_status() {
        return classify_provider_error(
            status,
            error.provider_response_body().unwrap_or_default(),
            Some(error.to_string()),
        );
    }
    if let Some(body) = error.provider_response_body() {
        return classify_provider_error(
            reqwest::StatusCode::BAD_REQUEST,
            body,
            Some(error.to_string()),
        );
    }

    match error {
        CompletionError::RequestError(_) | CompletionError::JsonError(_) => AppError::new(
            AppErrorKind::UnsupportedParameter,
            "Invalid provider request",
        )
        .with_detail(error.to_string()),
        CompletionError::HttpError(_) if !had_output => AppError::network(error),
        CompletionError::HttpError(_) | CompletionError::ProviderError(_) if had_output => {
            AppError::new(
                AppErrorKind::StreamInterrupted,
                "Provider stream was interrupted",
            )
            .with_detail(error.to_string())
        }
        CompletionError::HttpError(_) | CompletionError::ProviderError(_) => {
            AppError::network(error)
        }
        _ => AppError::new(AppErrorKind::Unknown, "Provider request failed")
            .with_detail(error.to_string()),
    }
}

pub(crate) fn classify_provider_error(
    status: reqwest::StatusCode,
    body: &str,
    detail: Option<String>,
) -> AppError {
    let lowercase = body.to_lowercase();
    let kind = match status {
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
            AppErrorKind::Authentication
        }
        reqwest::StatusCode::TOO_MANY_REQUESTS => AppErrorKind::RateLimited,
        reqwest::StatusCode::NOT_FOUND => AppErrorKind::ModelNotFound,
        status if status.is_server_error() => AppErrorKind::ProviderUnavailable,
        reqwest::StatusCode::BAD_REQUEST
            if lowercase.contains("context")
                && (lowercase.contains("length") || lowercase.contains("token")) =>
        {
            AppErrorKind::ContextLengthExceeded
        }
        reqwest::StatusCode::BAD_REQUEST
            if lowercase.contains("parameter")
                || lowercase.contains("unsupported")
                || lowercase.contains("invalid") =>
        {
            AppErrorKind::UnsupportedParameter
        }
        _ => AppErrorKind::Unknown,
    };
    let friendly = match kind {
        AppErrorKind::Authentication => "Authentication failed",
        AppErrorKind::ProviderUnavailable => "Provider is unavailable",
        AppErrorKind::ModelNotFound => "Model was not found",
        AppErrorKind::RateLimited => "Provider rate limit reached",
        AppErrorKind::ContextLengthExceeded => "Conversation exceeds the model context limit",
        AppErrorKind::UnsupportedParameter => "Provider rejected a generation parameter",
        _ => "Provider request failed",
    };
    AppError {
        kind,
        message: friendly.into(),
        detail: detail.or_else(|| (!body.is_empty()).then(|| body.to_string())),
    }
}

pub(crate) async fn emit_usage(
    usage: rig_core::completion::Usage,
    events: &Sender<GenerationEvent>,
) -> Result<bool, AppError> {
    if !usage.has_values() {
        return Ok(false);
    }
    events
        .send(GenerationEvent::UsageUpdated(TokenUsage {
            input_tokens: Some(usage.input_tokens),
            output_tokens: Some(usage.output_tokens),
            estimated: false,
        }))
        .await
        .map_err(|_| AppError::cancelled())?;
    Ok(true)
}

pub(crate) fn remove_keys(parameters: &mut Map<String, Value>, keys: &[&str]) {
    for key in keys {
        parameters.remove(*key);
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::time::Duration;

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    pub(crate) async fn server(
        status: &str,
        content_type: &str,
        chunks: Vec<(Duration, String)>,
    ) -> (String, oneshot::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let content_type = content_type.to_string();
        let content_length = chunks.iter().map(|(_, chunk)| chunk.len()).sum::<usize>();
        let (request_sender, request_receiver) = oneshot::channel();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;
            let _ = request_sender.send(String::from_utf8_lossy(&request).into_owned());
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            for (delay, chunk) in chunks {
                tokio::time::sleep(delay).await;
                if stream.write_all(chunk.as_bytes()).await.is_err() {
                    break;
                }
                let _ = stream.flush().await;
            }
        });

        (format!("http://{address}"), request_receiver)
    }

    pub(crate) fn fragmented(value: &str, width: usize) -> Vec<(Duration, String)> {
        value
            .as_bytes()
            .chunks(width)
            .map(|chunk| {
                (
                    Duration::from_millis(1),
                    String::from_utf8(chunk.to_vec()).unwrap(),
                )
            })
            .collect()
    }

    async fn read_request(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = stream.read(&mut buffer).await.unwrap_or_default();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            let Some(header_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
                continue;
            };
            let body_start = header_end + 4;
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or_default();
            if request.len() >= body_start + content_length {
                break;
            }
        }
        request
    }

    pub(crate) fn request_json(request: &str) -> Value {
        let body = request.split_once("\r\n\r\n").unwrap().1;
        serde_json::from_str(body).unwrap()
    }

    use serde_json::Value;
}
