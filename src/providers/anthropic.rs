use async_channel::Sender;
use rig_core::{
    client::{CompletionClient, VerifyClient},
    completion::Message,
    providers::anthropic as rig_anthropic,
};
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{GenerationError, GenerationErrorKind, GenerationEvent, GenerationRequest, Provider},
    providers::{
        insert_optional, merged_additional_parameters, remove_keys, sdk_base_url, sdk_headers,
        sdk_http_client, sdk_request, sdk_verify_error, stream_model,
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
    let mut sdk_request = sdk_request(&request, additional_parameters(&request)?)?;
    if sdk_request.max_tokens.is_none() {
        sdk_request.max_tokens = Some(4096);
    }
    stream_model(model, sdk_request, events, cancellation, true).await
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
    Ok(parameters)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{GenerationConfig, Model, ProviderKind};

    #[test]
    fn keeps_anthropic_parameters_at_the_expected_level() {
        let provider = Provider::new("Anthropic", ProviderKind::Anthropic);
        let mut model = Model::new(&provider.id, "model", "Model");
        model.capabilities.top_k = true;
        let mut config = GenerationConfig {
            top_p: Some(0.8),
            top_k: Some(40),
            stop_sequences: vec!["stop".into()],
            ..Default::default()
        };
        config.extra.insert("messages".into(), json!(["ignored"]));
        config.extra.insert("custom".into(), json!(true));
        let request = GenerationRequest {
            provider,
            model,
            system_prompt: String::new(),
            config,
            messages: vec![Message::user("Hello")],
            audio_duration_ms: 0,
            tools: Vec::new(),
        };

        assert_eq!(
            additional_parameters(&request).unwrap(),
            serde_json::from_value::<Map<String, Value>>(json!({
                "custom": true,
                "top_p": 0.8,
                "top_k": 40,
                "stop_sequences": ["stop"]
            }))
            .unwrap()
        );
    }
}
