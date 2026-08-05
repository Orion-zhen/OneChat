use std::collections::BTreeMap;

use async_channel::Sender;
use futures_util::StreamExt;
use reqwest::{
    Client, RequestBuilder, Response, StatusCode,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

use crate::{
    model::{MessageRole, Provider, TokenUsage},
    providers::{AppError, AppErrorKind, GenerationEvent, GenerationRequest},
};

pub async fn test_connection(provider: &Provider) -> Result<(), AppError> {
    let client = build_client(provider)?;
    let request = apply_headers(client.get(endpoint(provider, "models")?), provider)?;
    let response = request.send().await.map_err(AppError::network)?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(error_response(response).await)
    }
}

pub async fn stream(
    request: GenerationRequest,
    events: &Sender<GenerationEvent>,
    cancellation: CancellationToken,
) -> Result<(), AppError> {
    if cancellation.is_cancelled() {
        return Err(AppError::cancelled());
    }

    let client = build_client(&request.provider)?;
    let body = request_body(&request);
    let http_request = apply_headers(
        client.post(endpoint(&request.provider, "chat/completions")?),
        &request.provider,
    )?
    .json(&body);
    let response = tokio::select! {
        _ = cancellation.cancelled() => return Err(AppError::cancelled()),
        response = http_request.send() => response.map_err(AppError::network)?,
    };
    if !response.status().is_success() {
        return Err(error_response(response).await);
    }

    events
        .send(GenerationEvent::Started)
        .await
        .map_err(|_| AppError::cancelled())?;

    let mut decoder = SseDecoder::default();
    let mut bytes = response.bytes_stream();
    let mut completed = false;
    loop {
        let chunk = tokio::select! {
            _ = cancellation.cancelled() => return Err(AppError::cancelled()),
            chunk = bytes.next() => chunk,
        };
        let Some(chunk) = chunk else {
            break;
        };
        let chunk = chunk.map_err(|error| {
            AppError::new(
                AppErrorKind::StreamInterrupted,
                "Provider stream was interrupted",
            )
            .with_detail(error.to_string())
        })?;
        for data in decoder.push(&chunk) {
            if parse_data(&data, events).await? {
                completed = true;
                break;
            }
        }
        if completed {
            break;
        }
    }

    if completed {
        Ok(())
    } else {
        Err(AppError::new(
            AppErrorKind::StreamInterrupted,
            "Provider stream ended before completion",
        ))
    }
}

fn request_body(request: &GenerationRequest) -> Value {
    let capabilities = &request.model.capabilities;
    let config = &request.config;
    let mut body = config.extra.clone();
    let mut messages = Vec::new();
    if capabilities.system_prompt && !request.system_prompt.trim().is_empty() {
        messages.push(json!({"role": "system", "content": request.system_prompt}));
    }
    messages.extend(request.messages.iter().map(|message| {
        json!({
            "role": match message.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
            },
            "content": message.content,
        })
    }));

    body.insert("model".into(), json!(request.model.remote_id));
    body.insert("messages".into(), Value::Array(messages));
    body.insert("stream".into(), Value::Bool(true));
    body.insert("stream_options".into(), json!({"include_usage": true}));
    insert_optional(
        &mut body,
        "temperature",
        capabilities
            .temperature
            .then_some(config.temperature)
            .flatten(),
    );
    insert_optional(
        &mut body,
        "top_p",
        capabilities.top_p.then_some(config.top_p).flatten(),
    );
    insert_optional(
        &mut body,
        "top_k",
        capabilities.top_k.then_some(config.top_k).flatten(),
    );
    insert_optional(
        &mut body,
        "max_tokens",
        capabilities
            .max_output_tokens
            .then_some(config.max_output_tokens)
            .flatten(),
    );
    insert_optional(
        &mut body,
        "frequency_penalty",
        capabilities
            .frequency_penalty
            .then_some(config.frequency_penalty)
            .flatten(),
    );
    insert_optional(
        &mut body,
        "presence_penalty",
        capabilities
            .presence_penalty
            .then_some(config.presence_penalty)
            .flatten(),
    );
    insert_optional(
        &mut body,
        "seed",
        capabilities.seed.then_some(config.seed).flatten(),
    );
    if capabilities.stop_sequences && !config.stop_sequences.is_empty() {
        body.insert("stop".into(), json!(config.stop_sequences));
    }
    insert_optional(
        &mut body,
        "thinking_budget",
        capabilities
            .thinking_budget
            .then_some(config.thinking_budget)
            .flatten(),
    );
    Value::Object(body)
}

fn insert_optional<T: serde::Serialize>(
    body: &mut Map<String, Value>,
    key: &str,
    value: Option<T>,
) {
    if let Some(value) = value {
        body.insert(key.into(), json!(value));
    }
}

async fn parse_data(data: &str, events: &Sender<GenerationEvent>) -> Result<bool, AppError> {
    if data.trim() == "[DONE]" {
        events
            .send(GenerationEvent::Completed)
            .await
            .map_err(|_| AppError::cancelled())?;
        return Ok(true);
    }

    let value: Value = serde_json::from_str(data).map_err(|error| {
        AppError::new(AppErrorKind::StreamInterrupted, "Invalid SSE payload")
            .with_detail(error.to_string())
    })?;
    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Provider returned a stream error");
        return Err(classify_provider_error(
            StatusCode::BAD_REQUEST,
            message,
            Some(error.to_string()),
        ));
    }

    if let Some(usage) = value.get("usage").filter(|usage| !usage.is_null()) {
        events
            .send(GenerationEvent::UsageUpdated(TokenUsage {
                input_tokens: usage.get("prompt_tokens").and_then(Value::as_u64),
                output_tokens: usage.get("completion_tokens").and_then(Value::as_u64),
                estimated: false,
            }))
            .await
            .map_err(|_| AppError::cancelled())?;
    }

    if let Some(choices) = value.get("choices").and_then(Value::as_array) {
        for choice in choices {
            let Some(delta) = choice.get("delta") else {
                continue;
            };
            if let Some(text) = delta.get("content").and_then(Value::as_str)
                && !text.is_empty()
            {
                events
                    .send(GenerationEvent::TextDelta(text.into()))
                    .await
                    .map_err(|_| AppError::cancelled())?;
            }
            if let Some(thinking) = delta
                .get("reasoning_content")
                .or_else(|| delta.get("thinking"))
                .and_then(Value::as_str)
                && !thinking.is_empty()
            {
                events
                    .send(GenerationEvent::ThinkingDelta(thinking.into()))
                    .await
                    .map_err(|_| AppError::cancelled())?;
            }
        }
    }
    Ok(false)
}

fn build_client(provider: &Provider) -> Result<Client, AppError> {
    let mut builder = Client::builder().no_proxy();
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

fn apply_headers(request: RequestBuilder, provider: &Provider) -> Result<RequestBuilder, AppError> {
    let mut headers = header_map(&provider.headers)?;
    if !provider.api_key.trim().is_empty() {
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", provider.api_key)).map_err(|error| {
                AppError::new(AppErrorKind::Authentication, "Invalid API key header")
                    .with_detail(error.to_string())
            })?,
        );
    }
    Ok(request.headers(headers))
}

fn header_map(headers: &BTreeMap<String, String>) -> Result<HeaderMap, AppError> {
    let mut result = HeaderMap::new();
    for (name, value) in headers {
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
        result.insert(name, value);
    }
    Ok(result)
}

fn endpoint(provider: &Provider, suffix: &str) -> Result<String, AppError> {
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
    let suffix = suffix.trim_start_matches('/');
    let mut base = base.trim_end_matches('/');
    if base.ends_with(suffix) {
        return Ok(base.to_string());
    }
    for known_suffix in ["chat/completions", "models"] {
        if let Some(root) = base.strip_suffix(&format!("/{known_suffix}")) {
            base = root;
            break;
        }
    }
    Ok(format!("{base}/{suffix}"))
}

async fn error_response(response: Response) -> AppError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .pointer("/error/message")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| format!("Provider returned HTTP {status}"));
    classify_provider_error(status, &message, (!body.is_empty()).then_some(body))
}

fn classify_provider_error(status: StatusCode, message: &str, detail: Option<String>) -> AppError {
    let lowercase = message.to_lowercase();
    let kind = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => AppErrorKind::Authentication,
        StatusCode::TOO_MANY_REQUESTS => AppErrorKind::RateLimited,
        StatusCode::NOT_FOUND => AppErrorKind::ModelNotFound,
        status if status.is_server_error() => AppErrorKind::ProviderUnavailable,
        StatusCode::BAD_REQUEST
            if lowercase.contains("context")
                && (lowercase.contains("length") || lowercase.contains("token")) =>
        {
            AppErrorKind::ContextLengthExceeded
        }
        StatusCode::BAD_REQUEST
            if lowercase.contains("parameter") || lowercase.contains("unsupported") =>
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
        detail: detail.or_else(|| Some(message.into())),
    }
}

#[derive(Default)]
struct SseDecoder {
    buffer: Vec<u8>,
    data_lines: Vec<String>,
}

impl SseDecoder {
    fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(newline) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line = self.buffer.drain(..=newline).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line = String::from_utf8_lossy(&line);
            if line.is_empty() {
                if !self.data_lines.is_empty() {
                    events.push(self.data_lines.join("\n"));
                    self.data_lines.clear();
                }
            } else if let Some(data) = line.strip_prefix("data:") {
                self.data_lines
                    .push(data.strip_prefix(' ').unwrap_or(data).to_string());
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::model::{GenerationConfig, Model, ProviderKind};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    const SUCCESS_FIXTURE: &str = include_str!("../../tests/fixtures/openai_success.sse");
    const ERROR_FIXTURE: &str = include_str!("../../tests/fixtures/openai_error.sse");
    const INTERRUPTED_FIXTURE: &str = include_str!("../../tests/fixtures/openai_interrupted.sse");

    async fn test_server(status: &'static str, chunks: Vec<(Duration, &'static str)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).await;
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
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
        format!("http://{address}/v1")
    }

    fn request() -> GenerationRequest {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        let mut model = Model::new(&provider.id, "gpt-test", "GPT Test");
        model.capabilities.seed = true;
        GenerationRequest {
            provider,
            model,
            system_prompt: "Be concise".into(),
            config: GenerationConfig {
                temperature: Some(0.2),
                top_k: Some(40),
                seed: Some(7),
                extra: Map::from_iter([("reasoning_effort".into(), json!("high"))]),
                ..GenerationConfig::default()
            },
            messages: vec![crate::providers::ChatMessage {
                role: MessageRole::User,
                content: "Hello".into(),
            }],
        }
    }

    #[test]
    fn request_body_uses_model_capabilities_and_has_no_preset() {
        let body = request_body(&request());
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["temperature"], 0.2);
        assert_eq!(body["seed"], 7);
        assert!(body.get("top_k").is_none());
        assert_eq!(body["reasoning_effort"], "high");
        assert!(body.get("preset").is_none());
        assert_eq!(body["messages"][0]["role"], "system");
    }

    #[test]
    fn decoder_handles_every_byte_as_a_separate_chunk_and_multiline_data() {
        let input = b"event: message\r\ndata: {\"a\":\r\ndata: 1}\r\n\r\ndata: [DONE]\n\n";
        let mut decoder = SseDecoder::default();
        let mut events = Vec::new();
        for byte in input {
            events.extend(decoder.push(&[*byte]));
        }
        assert_eq!(events, vec!["{\"a\":\n1}", "[DONE]"]);
    }

    #[tokio::test]
    async fn fragmented_stream_emits_text_usage_and_completion() {
        let input = SUCCESS_FIXTURE;
        let (sender, receiver) = async_channel::unbounded();
        let mut decoder = SseDecoder::default();
        let mut completed = false;
        for chunk in input.as_bytes().chunks(3) {
            for data in decoder.push(chunk) {
                completed |= parse_data(&data, &sender).await.unwrap();
            }
        }
        drop(sender);
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        assert!(completed);
        assert_eq!(events[0], GenerationEvent::TextDelta("Hi".into()));
        assert_eq!(
            events[1],
            GenerationEvent::UsageUpdated(TokenUsage {
                input_tokens: Some(3),
                output_tokens: Some(2),
                estimated: false,
            })
        );
        assert_eq!(events[2], GenerationEvent::Completed);
    }

    #[tokio::test]
    async fn stream_error_fixture_maps_provider_errors() {
        let (sender, _receiver) = async_channel::unbounded();
        let mut decoder = SseDecoder::default();
        let data = decoder.push(ERROR_FIXTURE.as_bytes()).remove(0);
        let error = parse_data(&data, &sender).await.unwrap_err();
        assert_eq!(error.kind, AppErrorKind::UnsupportedParameter);
    }

    #[test]
    fn maps_common_http_errors() {
        assert_eq!(
            classify_provider_error(StatusCode::UNAUTHORIZED, "bad key", None).kind,
            AppErrorKind::Authentication
        );
        assert_eq!(
            classify_provider_error(StatusCode::TOO_MANY_REQUESTS, "slow down", None).kind,
            AppErrorKind::RateLimited
        );
        assert_eq!(
            classify_provider_error(StatusCode::BAD_REQUEST, "context length exceeded", None).kind,
            AppErrorKind::ContextLengthExceeded
        );
        assert_eq!(
            classify_provider_error(StatusCode::NOT_FOUND, "unknown model", None).kind,
            AppErrorKind::ModelNotFound
        );
        assert_eq!(
            classify_provider_error(StatusCode::SERVICE_UNAVAILABLE, "offline", None).kind,
            AppErrorKind::ProviderUnavailable
        );
        assert_eq!(
            AppError::network("disconnected").kind,
            AppErrorKind::Network
        );
    }

    #[tokio::test]
    async fn network_stream_handles_fragmented_sse_and_usage() {
        let endpoint = test_server(
            "200 OK",
            vec![
                (
                    Duration::ZERO,
                    "data: {\"choices\":[{\"delta\":{\"content\":\"Hel",
                ),
                (
                    Duration::from_millis(2),
                    "lo\"}}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":1}}\n\n",
                ),
                (Duration::from_millis(2), "data: [DONE]\n\n"),
            ],
        )
        .await;
        let mut request = request();
        request.provider.endpoint = endpoint;
        let (sender, receiver) = async_channel::unbounded();

        stream(request, &sender, CancellationToken::new())
            .await
            .unwrap();
        drop(sender);
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        assert_eq!(events[0], GenerationEvent::Started);
        assert!(events.contains(&GenerationEvent::TextDelta("Hello".into())));
        assert!(events.contains(&GenerationEvent::Completed));
    }

    #[tokio::test]
    async fn network_stream_reports_http_errors_and_early_eof() {
        let endpoint = test_server(
            "401 Unauthorized",
            vec![(Duration::ZERO, "{\"error\":{\"message\":\"bad key\"}}")],
        )
        .await;
        let mut unauthorized = request();
        unauthorized.provider.endpoint = endpoint;
        let (sender, _) = async_channel::unbounded();
        assert_eq!(
            stream(unauthorized, &sender, CancellationToken::new())
                .await
                .unwrap_err()
                .kind,
            AppErrorKind::Authentication
        );

        let endpoint = test_server("200 OK", vec![(Duration::ZERO, INTERRUPTED_FIXTURE)]).await;
        let mut interrupted = request();
        interrupted.provider.endpoint = endpoint;
        let (sender, _receiver) = async_channel::unbounded();
        assert_eq!(
            stream(interrupted, &sender, CancellationToken::new())
                .await
                .unwrap_err()
                .kind,
            AppErrorKind::StreamInterrupted
        );
    }

    #[tokio::test]
    async fn cancellation_interrupts_an_open_stream() {
        let endpoint =
            test_server("200 OK", vec![(Duration::from_secs(2), "data: [DONE]\n\n")]).await;
        let mut request = request();
        request.provider.endpoint = endpoint;
        let (sender, _) = async_channel::unbounded();
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move { stream(request, &sender, task_cancellation).await });
        tokio::time::sleep(Duration::from_millis(30)).await;
        cancellation.cancel();

        assert_eq!(
            task.await.unwrap().unwrap_err().kind,
            AppErrorKind::UserCancelled
        );
    }

    #[tokio::test]
    async fn models_endpoint_connection_test_succeeds() {
        let endpoint = test_server("200 OK", vec![(Duration::ZERO, "{\"data\":[]}")]).await;
        let mut provider = Provider::new("Local", ProviderKind::OpenAiCompatible);
        provider.endpoint = endpoint;
        test_connection(&provider).await.unwrap();
    }

    #[test]
    fn endpoint_accepts_a_complete_custom_path() {
        let mut provider = Provider::new("Local", ProviderKind::OpenAiCompatible);
        provider.endpoint = "http://localhost:8080/v1/chat/completions".into();
        assert_eq!(
            endpoint(&provider, "chat/completions").unwrap(),
            provider.endpoint
        );
        assert_eq!(
            endpoint(&provider, "models").unwrap(),
            "http://localhost:8080/v1/models"
        );
    }
}
