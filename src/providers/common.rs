use async_channel::Sender;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use rig_core::{
    OneOrMany,
    completion::{CompletionRequest, GetTokenUsage, Message},
    streaming::{StreamedAssistantContent, StreamingCompletionResponse},
};
use serde::Serialize;
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

use crate::domain::{
    GenerationError, GenerationErrorKind, GenerationEvent, GenerationRequest, Provider,
    ProviderKind, TokenUsage, merge_json_patch,
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
            for suffix in ["/responses", "/chat/completions", "/models"] {
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

pub(crate) fn merged_additional_parameters(
    request: &GenerationRequest,
) -> Result<Map<String, Value>, GenerationError> {
    let mut parameters = request.config.extra.clone();
    if let Some(reasoning) = &request.model.reasoning {
        let (_, patch) = reasoning
            .resolve_patch(request.config.reasoning_preset.as_deref())
            .map_err(|detail| {
                GenerationError::new(
                    GenerationErrorKind::UnsupportedParameter,
                    "Invalid model reasoning configuration",
                )
                .with_detail(detail)
            })?;
        merge_json_patch(&mut parameters, patch);
    }
    Ok(parameters)
}

pub(crate) fn sdk_request(
    request: &GenerationRequest,
    additional_params: Map<String, Value>,
) -> Result<CompletionRequest, GenerationError> {
    let chat_history = OneOrMany::many(request.messages.clone()).map_err(|_| {
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
        tools: if capabilities.tools {
            request.tools.clone()
        } else {
            Vec::new()
        },
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

pub(crate) async fn consume_stream<R>(
    mut response: StreamingCompletionResponse<R>,
    events: &Sender<GenerationEvent>,
    cancellation: CancellationToken,
    require_usage: bool,
    mut validate_final: impl FnMut(&R) -> Result<(), GenerationError>,
) -> Result<Message, GenerationError>
where
    R: Clone + Unpin + GetTokenUsage,
{
    events
        .send(GenerationEvent::Started)
        .await
        .map_err(|_| GenerationError::cancelled())?;

    let mut had_output = false;
    let mut had_reasoning_delta = false;
    let mut saw_final = false;
    let mut saw_usage = false;
    loop {
        let item = tokio::select! {
            _ = cancellation.cancelled() => return Err(GenerationError::cancelled()),
            item = response.next() => item,
        };
        let Some(item) = item else { break };
        match item.map_err(|error| super::sdk_completion_error(error, had_output))? {
            StreamedAssistantContent::Text(text) if !text.text().is_empty() => {
                had_output = true;
                events
                    .send(GenerationEvent::TextDelta(text.text().to_string()))
                    .await
                    .map_err(|_| GenerationError::cancelled())?;
            }
            StreamedAssistantContent::ReasoningDelta { reasoning, .. } if !reasoning.is_empty() => {
                had_output = true;
                had_reasoning_delta = true;
                events
                    .send(GenerationEvent::ThinkingDelta(reasoning))
                    .await
                    .map_err(|_| GenerationError::cancelled())?;
            }
            StreamedAssistantContent::Reasoning(reasoning) if !had_reasoning_delta => {
                let text = reasoning
                    .content
                    .iter()
                    .filter_map(|content| match content {
                        rig_core::message::ReasoningContent::Text { text, .. }
                        | rig_core::message::ReasoningContent::Summary(text) => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<String>();
                if !text.is_empty() {
                    had_output = true;
                    events
                        .send(GenerationEvent::ThinkingDelta(text))
                        .await
                        .map_err(|_| GenerationError::cancelled())?;
                }
            }
            StreamedAssistantContent::ToolCall { .. }
            | StreamedAssistantContent::ToolCallDelta { .. } => {
                had_output = true;
                events
                    .send(GenerationEvent::ProviderOutput)
                    .await
                    .map_err(|_| GenerationError::cancelled())?;
            }
            StreamedAssistantContent::Final(final_response) => {
                validate_final(&final_response)?;
                saw_final = true;
                saw_usage |= emit_usage(final_response.token_usage(), events).await?;
            }
            _ => {}
        }
    }

    if !saw_final || (require_usage && !saw_usage) || (had_output && !saw_usage) {
        return Err(GenerationError::new(
            GenerationErrorKind::StreamInterrupted,
            "Provider stream ended before completion",
        ));
    }
    Ok(Message::Assistant {
        id: response.message_id.clone(),
        content: response.choice.clone(),
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
