use std::collections::HashSet;

use async_channel::Sender;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use rig_core::{
    completion::{CompletionModel, CompletionRequest, FinishReason, Message},
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
    if request.messages.is_empty() {
        return Err(GenerationError::new(
            GenerationErrorKind::UnsupportedParameter,
            "At least one chat message is required",
        ));
    }
    let capabilities = &request.model.capabilities;
    let mut chat_history = Vec::with_capacity(request.messages.len() + 1);
    if !request.system_prompt.trim().is_empty() {
        chat_history.push(Message::System {
            content: request.system_prompt.clone(),
        });
    }
    chat_history.extend(request.messages.clone());

    let sdk_request = CompletionRequest {
        model: Some(request.model.remote_id.clone()),
        preamble: None,
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
    };
    sdk_request
        .validate_message_content()
        .map_err(|error| super::sdk_completion_error(error, false))?;
    Ok(sdk_request)
}

pub(crate) async fn stream_model<M>(
    model: M,
    request: CompletionRequest,
    events: &Sender<GenerationEvent>,
    cancellation: CancellationToken,
    require_usage: bool,
) -> Result<Message, GenerationError>
where
    M: CompletionModel,
{
    if cancellation.is_cancelled() {
        return Err(GenerationError::cancelled());
    }
    let response = tokio::select! {
        _ = cancellation.cancelled() => return Err(GenerationError::cancelled()),
        response = model.stream(request) => {
            response.map_err(|error| super::sdk_completion_error(error, false))?
        }
    };

    consume_stream(response, events, cancellation, require_usage).await
}

async fn consume_stream(
    mut response: StreamingCompletionResponse,
    events: &Sender<GenerationEvent>,
    cancellation: CancellationToken,
    require_usage: bool,
) -> Result<Message, GenerationError> {
    events
        .send(GenerationEvent::Started)
        .await
        .map_err(|_| GenerationError::cancelled())?;

    let mut had_output = false;
    let mut reasoning_delta_ids = HashSet::new();
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
            StreamedAssistantContent::ReasoningDelta {
                id,
                provider_id,
                reasoning,
            } if !reasoning.is_empty() => {
                had_output = true;
                reasoning_delta_ids.insert(id);
                events
                    .send(GenerationEvent::ThinkingDelta {
                        provider_id,
                        delta: reasoning,
                    })
                    .await
                    .map_err(|_| GenerationError::cancelled())?;
            }
            StreamedAssistantContent::Reasoning { reasoning, id } => {
                if !reasoning_delta_ids.contains(&id) {
                    let text = reasoning
                        .content
                        .iter()
                        .filter_map(|content| match content {
                            rig_core::message::ReasoningContent::Text { text, .. }
                            | rig_core::message::ReasoningContent::Summary(text) => {
                                Some(text.as_str())
                            }
                            _ => None,
                        })
                        .collect::<String>();
                    if !text.is_empty() {
                        had_output = true;
                        events
                            .send(GenerationEvent::ThinkingDelta {
                                provider_id: reasoning.id,
                                delta: text,
                            })
                            .await
                            .map_err(|_| GenerationError::cancelled())?;
                    }
                }
            }
            StreamedAssistantContent::ToolCall {
                tool_call,
                internal_call_id,
            } => {
                had_output = true;
                events
                    .send(GenerationEvent::ToolCallObserved {
                        stream_call_id: internal_call_id,
                        call_id: Some(tool_call.id.into_string()),
                    })
                    .await
                    .map_err(|_| GenerationError::cancelled())?;
            }
            StreamedAssistantContent::ToolCallDelta {
                internal_call_id, ..
            } => {
                had_output = true;
                events
                    .send(GenerationEvent::ToolCallObserved {
                        stream_call_id: internal_call_id,
                        call_id: None,
                    })
                    .await
                    .map_err(|_| GenerationError::cancelled())?;
            }
            StreamedAssistantContent::Text(_)
            | StreamedAssistantContent::ReasoningDelta { .. }
            | StreamedAssistantContent::Final(_)
            | StreamedAssistantContent::Unknown(_) => {}
        }
    }

    let Some(final_response) = response.response.as_ref() else {
        return Err(GenerationError::new(
            GenerationErrorKind::StreamInterrupted,
            "Provider stream ended before completion",
        ));
    };
    validate_finish_reason(final_response.finish_reason.as_ref())?;
    let usage = final_response.usage;
    let has_usage = emit_usage(usage, events).await?;
    if (require_usage || had_output) && !has_usage {
        return Err(GenerationError::new(
            GenerationErrorKind::StreamInterrupted,
            "Provider stream ended before completion",
        ));
    }
    Ok(Message::Assistant {
        id: response.message_id,
        content: response.choice,
    })
}

fn validate_finish_reason(reason: Option<&FinishReason>) -> Result<(), GenerationError> {
    match reason {
        Some(FinishReason::ContentFilter) => Err(GenerationError::new(
            GenerationErrorKind::Unknown,
            "Provider filtered the response",
        )),
        Some(FinishReason::Other(reason)) => Err(GenerationError::new(
            GenerationErrorKind::Unknown,
            "Provider stopped without completing the response",
        )
        .with_detail(format!("finish_reason={reason}"))),
        Some(FinishReason::Stop | FinishReason::Length | FinishReason::ToolCalls) | None => Ok(()),
    }
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

#[cfg(test)]
mod tests;
