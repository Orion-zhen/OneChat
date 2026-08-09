use async_channel::Sender;
use rig_core::{
    client::{CompletionClient, VerifyClient},
    completion::{CompletionModel, Message},
    providers::gemini::{self as rig_gemini, completion::gemini_api_types::FinishReason},
};
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{GenerationError, GenerationErrorKind, GenerationEvent, GenerationRequest, Provider},
    providers::{
        consume_stream, insert_optional, merged_additional_parameters, remove_keys, sdk_base_url,
        sdk_completion_error, sdk_headers, sdk_http_client, sdk_request, sdk_verify_error,
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
    let sdk_request = sdk_request(&request, additional_parameters(&request)?)?;
    let response = tokio::select! {
        _ = cancellation.cancelled() => return Err(GenerationError::cancelled()),
        response = model.stream(sdk_request) => response.map_err(|error| sdk_completion_error(error, false))?,
    };

    consume_stream(
        response,
        events,
        cancellation,
        true,
        |final_response| match final_response.finish_reason {
            Some(FinishReason::Stop | FinishReason::MaxTokens) => Ok(()),
            Some(ref reason) => Err(GenerationError::new(
                GenerationErrorKind::Unknown,
                "Gemini stopped without completing the response",
            )
            .with_detail(format!(
                "finish_reason={reason:?}, message={}",
                final_response.finish_message.as_deref().unwrap_or("none")
            ))),
            None => Err(GenerationError::new(
                GenerationErrorKind::StreamInterrupted,
                "Provider stream ended before completion",
            )),
        },
    )
    .await
}

fn additional_parameters(
    request: &GenerationRequest,
) -> Result<Map<String, Value>, GenerationError> {
    let capabilities = &request.model.capabilities;
    let config = &request.config;
    let mut parameters = merged_additional_parameters(request)?;
    let generation_config = parameters
        .remove("generationConfig")
        .or_else(|| parameters.remove("generation_config"));
    let mut generation_config = match generation_config {
        Some(Value::Object(config)) => config,
        Some(_) => {
            return Err(GenerationError::new(
                GenerationErrorKind::UnsupportedParameter,
                "Gemini generationConfig must be a JSON object",
            ));
        }
        None => Map::new(),
    };
    remove_keys(
        &mut parameters,
        &[
            "model",
            "contents",
            "systemInstruction",
            "stream",
            "tools",
            "toolConfig",
            "tool_config",
            "tool_choice",
        ],
    );
    remove_keys(
        &mut generation_config,
        &[
            "temperature",
            "topP",
            "topK",
            "maxOutputTokens",
            "frequencyPenalty",
            "presencePenalty",
            "seed",
            "stopSequences",
        ],
    );
    insert_optional(
        &mut generation_config,
        "topP",
        capabilities.top_p.then_some(config.top_p).flatten(),
    );
    insert_optional(
        &mut generation_config,
        "topK",
        capabilities.top_k.then_some(config.top_k).flatten(),
    );
    insert_optional(
        &mut generation_config,
        "frequencyPenalty",
        capabilities
            .frequency_penalty
            .then_some(config.frequency_penalty)
            .flatten(),
    );
    insert_optional(
        &mut generation_config,
        "presencePenalty",
        capabilities
            .presence_penalty
            .then_some(config.presence_penalty)
            .flatten(),
    );
    if capabilities.stop_sequences && !config.stop_sequences.is_empty() {
        generation_config.insert("stopSequences".into(), json!(config.stop_sequences));
    }
    if !generation_config.is_empty() {
        parameters.insert("generationConfig".into(), Value::Object(generation_config));
    }
    Ok(parameters)
}

fn build_client(provider: &Provider) -> Result<rig_gemini::Client, GenerationError> {
    rig_gemini::Client::builder()
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
