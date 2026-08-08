use async_channel::Sender;
use rig_core::{
    client::{CompletionClient, VerifyClient},
    completion::{CompletionModel, Message},
    providers::anthropic as rig_anthropic,
};
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{GenerationError, GenerationErrorKind, GenerationEvent, GenerationRequest, Provider},
    providers::{
        consume_stream, insert_optional, remove_keys, sdk_base_url, sdk_completion_error,
        sdk_headers, sdk_http_client, sdk_request, sdk_verify_error,
    },
};

pub async fn test_connection(provider: &Provider) -> Result<(), GenerationError> {
    build_client(provider)?
        .verify()
        .await
        .map_err(sdk_verify_error)
}

pub async fn stream(
    request: GenerationRequest,
    events: &Sender<GenerationEvent>,
    cancellation: CancellationToken,
) -> Result<Message, GenerationError> {
    if cancellation.is_cancelled() {
        return Err(GenerationError::cancelled());
    }

    let client = build_client(&request.provider)?;
    let model = client.completion_model(request.model.remote_id.clone());
    let mut sdk_request = sdk_request(&request, additional_parameters(&request))?;
    if sdk_request.max_tokens.is_none() {
        sdk_request.max_tokens = Some(4096);
    }
    let response = tokio::select! {
        _ = cancellation.cancelled() => return Err(GenerationError::cancelled()),
        response = model.stream(sdk_request) => response.map_err(|error| sdk_completion_error(error, false))?,
    };

    consume_stream(response, events, cancellation, true, |_| Ok(())).await
}

fn additional_parameters(request: &GenerationRequest) -> Map<String, Value> {
    let capabilities = &request.model.capabilities;
    let config = &request.config;
    let mut parameters = config.extra.clone();
    remove_keys(
        &mut parameters,
        &[
            "model",
            "messages",
            "system",
            "stream",
            "temperature",
            "top_p",
            "top_k",
            "max_tokens",
            "frequency_penalty",
            "presence_penalty",
            "seed",
            "stop",
            "stop_sequences",
            "thinking",
            "thinking_budget",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
        ],
    );
    insert_optional(
        &mut parameters,
        "top_p",
        capabilities.top_p.then_some(config.top_p).flatten(),
    );
    insert_optional(
        &mut parameters,
        "top_k",
        capabilities.top_k.then_some(config.top_k).flatten(),
    );
    if capabilities.stop_sequences && !config.stop_sequences.is_empty() {
        parameters.insert("stop_sequences".into(), json!(config.stop_sequences));
    }
    if capabilities.thinking_budget
        && let Some(budget) = config.thinking_budget
    {
        parameters.insert(
            "thinking".into(),
            json!({"type": "enabled", "budget_tokens": budget}),
        );
    }
    parameters
}

fn build_client(provider: &Provider) -> Result<rig_anthropic::Client, GenerationError> {
    rig_anthropic::Client::builder()
        .api_key(provider.api_key.clone())
        .base_url(sdk_base_url(provider)?)
        .http_headers(sdk_headers(provider)?)
        .http_client(sdk_http_client(provider)?)
        .build()
        .map_err(|error| {
            GenerationError::new(
                GenerationErrorKind::UnsupportedParameter,
                "Invalid provider configuration",
            )
            .with_detail(error.to_string())
        })
}
