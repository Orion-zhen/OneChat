use async_channel::Sender;
use rig_core::{
    client::{CompletionClient, VerifyClient},
    completion::Message,
    providers::gemini::{self as rig_gemini, completion::gemini_api_types::FinishReason},
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
    let sdk_request = sdk_request(&request, additional_parameters(&request)?)?;
    stream_model(
        model,
        sdk_request,
        events,
        cancellation,
        true,
        validate_final_response,
    )
    .await
}

fn validate_final_response(
    response: &rig_gemini::streaming::StreamingCompletionResponse,
) -> Result<(), GenerationError> {
    match response.finish_reason {
        Some(FinishReason::Stop | FinishReason::MaxTokens) => Ok(()),
        Some(ref reason) => Err(GenerationError::new(
            GenerationErrorKind::Unknown,
            "Gemini stopped without completing the response",
        )
        .with_detail(format!(
            "finish_reason={reason:?}, message={}",
            response.finish_message.as_deref().unwrap_or("none")
        ))),
        None => Err(GenerationError::new(
            GenerationErrorKind::StreamInterrupted,
            "Provider stream ended before completion",
        )),
    }
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
            "responseModalities",
            "response_modalities",
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

#[cfg(test)]
mod tests {
    use super::*;
    use rig_core::{
        OneOrMany,
        message::{AudioMediaType, UserContent},
        providers::gemini::{
            completion::gemini_api_types::Content,
            streaming::{PartialUsage, StreamingCompletionResponse},
        },
    };

    fn final_response(
        finish_reason: Option<FinishReason>,
        finish_message: Option<&str>,
    ) -> StreamingCompletionResponse {
        StreamingCompletionResponse {
            usage_metadata: PartialUsage::default(),
            finish_reason,
            finish_message: finish_message.map(str::to_string),
            model_version: None,
        }
    }

    #[test]
    fn accepts_only_successful_gemini_finish_reasons() {
        for reason in [FinishReason::Stop, FinishReason::MaxTokens] {
            assert!(validate_final_response(&final_response(Some(reason), None)).is_ok());
        }

        let error = validate_final_response(&final_response(
            Some(FinishReason::Safety),
            Some("unsafe content"),
        ))
        .unwrap_err();
        assert_eq!(error.kind, GenerationErrorKind::Unknown);
        assert_eq!(
            error.detail.as_deref(),
            Some("finish_reason=Safety, message=unsafe content")
        );

        let error = validate_final_response(&final_response(None, None)).unwrap_err();
        assert_eq!(error.kind, GenerationErrorKind::StreamInterrupted);
    }

    #[test]
    fn strips_audio_output_modalities() {
        let provider = Provider::new("Gemini", crate::domain::ProviderKind::Gemini);
        let model = crate::domain::Model::new(&provider.id, "model", "Model");
        let mut config = crate::domain::GenerationConfig::default();
        config.extra.insert(
            "generationConfig".into(),
            json!({ "responseModalities": ["TEXT", "AUDIO"] }),
        );
        let request = GenerationRequest {
            provider,
            model,
            system_prompt: String::new(),
            config,
            messages: vec![Message::user("Hello")],
            audio_duration_ms: 0,
            tools: Vec::new(),
        };

        let parameters = additional_parameters(&request).unwrap();
        assert!(parameters.get("generationConfig").is_none());
    }

    #[test]
    fn serializes_ordered_wav_and_mp3_inline_audio_without_audio_output() {
        let message = Message::User {
            content: OneOrMany::many(vec![
                UserContent::text("First"),
                UserContent::audio("d2F2", Some(AudioMediaType::WAV)),
                UserContent::text("Second"),
                UserContent::audio("bXAz", Some(AudioMediaType::MP3)),
            ])
            .unwrap(),
        };
        let value = serde_json::to_value(Content::try_from(message).unwrap()).unwrap();

        assert_eq!(
            value["parts"][0],
            json!({ "thought": false, "text": "First" })
        );
        assert_eq!(
            value["parts"][1],
            json!({
                "thought": false,
                "inlineData": { "mimeType": "audio/wav", "data": "d2F2" }
            })
        );
        assert_eq!(
            value["parts"][2],
            json!({ "thought": false, "text": "Second" })
        );
        assert_eq!(
            value["parts"][3],
            json!({
                "thought": false,
                "inlineData": { "mimeType": "audio/mp3", "data": "bXAz" }
            })
        );
        assert!(!value.to_string().contains("responseModalities"));
    }
}
