use async_channel::Sender;
use rig_core::{
    client::{CompletionClient, VerifyClient},
    completion::Message,
    providers::openai as rig_openai,
};
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{
        GenerationError, GenerationErrorKind, GenerationEvent, GenerationRequest, Provider,
        ProviderKind,
    },
    providers::{
        insert_optional, merged_additional_parameters, remove_keys, sdk_base_url, sdk_headers,
        sdk_http_client, sdk_request, sdk_verify_error, stream_model,
    },
};

type OpenAiClient = rig_openai::Client<reqwest::Client>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OpenAiApi {
    Responses,
    ChatCompletions,
}

fn request_api(request: &GenerationRequest) -> OpenAiApi {
    match request.provider.kind {
        ProviderKind::OpenAi if request.model.capabilities.audio_input => {
            OpenAiApi::ChatCompletions
        }
        ProviderKind::OpenAi => OpenAiApi::Responses,
        ProviderKind::OpenAiCompatible => OpenAiApi::ChatCompletions,
        _ => unreachable!("request_api called for a non-OpenAI provider"),
    }
}

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
    let model_id = request.model.remote_id.clone();
    let sdk_request = sdk_request(&request, additional_parameters(&request)?)?;
    match request_api(&request) {
        OpenAiApi::Responses => {
            stream_model(
                client.completion_model(model_id),
                sdk_request,
                events,
                cancellation,
                false,
            )
            .await
        }
        OpenAiApi::ChatCompletions => {
            stream_model(
                client.completions_api().completion_model(model_id),
                sdk_request,
                events,
                cancellation,
                false,
            )
            .await
        }
    }
}

fn additional_parameters(
    request: &GenerationRequest,
) -> Result<Map<String, Value>, GenerationError> {
    let capabilities = &request.model.capabilities;
    let config = &request.config;
    let mut parameters = merged_additional_parameters(request)?;
    remove_keys(
        &mut parameters,
        &[
            "model",
            "messages",
            "stream",
            "temperature",
            "top_p",
            "top_k",
            "max_tokens",
            "max_completion_tokens",
            "frequency_penalty",
            "presence_penalty",
            "seed",
            "stop",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
            "modalities",
            "audio",
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
    insert_optional(
        &mut parameters,
        "frequency_penalty",
        capabilities
            .frequency_penalty
            .then_some(config.frequency_penalty)
            .flatten(),
    );
    insert_optional(
        &mut parameters,
        "presence_penalty",
        capabilities
            .presence_penalty
            .then_some(config.presence_penalty)
            .flatten(),
    );
    insert_optional(
        &mut parameters,
        "seed",
        capabilities.seed.then_some(config.seed).flatten(),
    );
    if capabilities.stop_sequences && !config.stop_sequences.is_empty() {
        parameters.insert("stop".into(), json!(config.stop_sequences));
    }
    Ok(parameters)
}

fn build_client(provider: &Provider) -> Result<OpenAiClient, GenerationError> {
    rig_openai::Client::builder()
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

#[cfg(test)]
mod tests;
