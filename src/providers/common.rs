use async_channel::Sender;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use rig_core::{
    OneOrMany,
    completion::{AssistantContent, CompletionRequest, Message},
    message::{ReasoningContent, UserContent},
};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::domain::{
    GenerationError, GenerationErrorKind, GenerationEvent, GenerationRequest, MessageRole,
    Provider, ProviderKind, TokenUsage,
};

pub(crate) fn sdk_http_client(provider: &Provider) -> Result<reqwest::Client, GenerationError> {
    let mut builder = reqwest::Client::builder().no_proxy();
    if let Some(proxy) = provider
        .proxy
        .as_deref()
        .filter(|proxy| !proxy.trim().is_empty())
    {
        builder = builder.proxy(reqwest::Proxy::all(proxy).map_err(|error| {
            GenerationError::new(
                GenerationErrorKind::UnsupportedParameter,
                "Invalid proxy URL",
            )
            .with_detail(error.to_string())
        })?);
    }
    builder.build().map_err(GenerationError::network)
}

pub(crate) fn sdk_headers(provider: &Provider) -> Result<HeaderMap, GenerationError> {
    provider
        .headers
        .iter()
        .map(|(name, value)| {
            let name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                GenerationError::new(
                    GenerationErrorKind::UnsupportedParameter,
                    "Invalid custom header name",
                )
                .with_detail(error.to_string())
            })?;
            let value = HeaderValue::from_str(value).map_err(|error| {
                GenerationError::new(
                    GenerationErrorKind::UnsupportedParameter,
                    "Invalid custom header value",
                )
                .with_detail(error.to_string())
            })?;
            Ok((name, value))
        })
        .collect()
}

pub(crate) fn sdk_base_url(provider: &Provider) -> Result<String, GenerationError> {
    let base = if provider.endpoint.trim().is_empty() {
        provider.kind.default_endpoint()
    } else {
        provider.endpoint.trim()
    };
    if base.is_empty() {
        return Err(GenerationError::new(
            GenerationErrorKind::UnsupportedParameter,
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
) -> Result<CompletionRequest, GenerationError> {
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
        GenerationError::new(
            GenerationErrorKind::UnsupportedParameter,
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

pub(crate) async fn emit_usage(
    usage: rig_core::completion::Usage,
    events: &Sender<GenerationEvent>,
) -> Result<bool, GenerationError> {
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
        .map_err(|_| GenerationError::cancelled())?;
    Ok(true)
}

pub(crate) fn remove_keys(parameters: &mut Map<String, Value>, keys: &[&str]) {
    for key in keys {
        parameters.remove(*key);
    }
}

pub(crate) fn insert_optional<T: Serialize>(
    parameters: &mut Map<String, Value>,
    key: &str,
    value: Option<T>,
) {
    if let Some(value) = value {
        parameters.insert(key.into(), json!(value));
    }
}

pub(crate) fn reasoning_text(content: &[ReasoningContent]) -> String {
    content
        .iter()
        .filter_map(|content| match content {
            ReasoningContent::Text { text, .. } | ReasoningContent::Summary(text) => {
                Some(text.as_str())
            }
            _ => None,
        })
        .collect()
}
