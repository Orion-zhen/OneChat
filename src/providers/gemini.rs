use async_channel::Sender;
use futures_util::StreamExt;
use rig_core::{
    client::{CompletionClient, VerifyClient},
    completion::{CompletionModel, GetTokenUsage},
    providers::gemini::{self as rig_gemini, completion::gemini_api_types::FinishReason},
    streaming::StreamedAssistantContent,
};
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{GenerationError, GenerationErrorKind, GenerationEvent, GenerationRequest, Provider},
    providers::{
        emit_usage, insert_optional, reasoning_text, remove_keys, sdk_base_url,
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
) -> Result<(), GenerationError> {
    if cancellation.is_cancelled() {
        return Err(GenerationError::cancelled());
    }

    let client = build_client(&request.provider)?;
    let model = client.completion_model(request.model.remote_id.clone());
    let sdk_request = sdk_request(&request, additional_parameters(&request)?)?;
    let mut response = tokio::select! {
        _ = cancellation.cancelled() => return Err(GenerationError::cancelled()),
        response = model.stream(sdk_request) => response.map_err(|error| sdk_completion_error(error, false))?,
    };

    events
        .send(GenerationEvent::Started)
        .await
        .map_err(|_| GenerationError::cancelled())?;

    let mut had_output = false;
    let mut had_reasoning_delta = false;
    let mut saw_final = false;
    let mut saw_usage = false;
    let mut completed = false;
    loop {
        let item = tokio::select! {
            _ = cancellation.cancelled() => return Err(GenerationError::cancelled()),
            item = response.next() => item,
        };
        let Some(item) = item else { break };
        match item.map_err(|error| sdk_completion_error(error, had_output))? {
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
                let text = reasoning_text(&reasoning.content);
                if !text.is_empty() {
                    had_output = true;
                    events
                        .send(GenerationEvent::ThinkingDelta(text))
                        .await
                        .map_err(|_| GenerationError::cancelled())?;
                }
            }
            StreamedAssistantContent::Final(final_response) => {
                saw_final = true;
                saw_usage |= emit_usage(final_response.token_usage(), events).await?;
                completed = match final_response.finish_reason {
                    Some(FinishReason::Stop | FinishReason::MaxTokens) => true,
                    Some(reason) => {
                        return Err(GenerationError::new(
                            GenerationErrorKind::Unknown,
                            "Gemini stopped without completing the response",
                        )
                        .with_detail(format!(
                            "finish_reason={reason:?}, message={}",
                            final_response.finish_message.as_deref().unwrap_or("none")
                        )));
                    }
                    None => false,
                };
            }
            _ => {}
        }
    }

    if !saw_final || !completed || !saw_usage {
        return Err(GenerationError::new(
            GenerationErrorKind::StreamInterrupted,
            "Provider stream ended before completion",
        ));
    }
    events
        .send(GenerationEvent::Completed)
        .await
        .map_err(|_| GenerationError::cancelled())?;
    Ok(())
}

fn additional_parameters(
    request: &GenerationRequest,
) -> Result<Map<String, Value>, GenerationError> {
    let capabilities = &request.model.capabilities;
    let config = &request.config;
    let mut parameters = config.extra.clone();
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
    let raw_thinking = generation_config.remove("thinkingConfig");
    remove_keys(
        &mut parameters,
        &["model", "contents", "systemInstruction", "stream"],
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
            "thinkingBudget",
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
    if capabilities.thinking
        && let Some(thinking) = raw_thinking
    {
        generation_config.insert("thinkingConfig".into(), thinking);
    }
    if capabilities.thinking_budget
        && let Some(budget) = config.thinking_budget
    {
        let budget = u32::try_from(budget).map_err(|error| {
            GenerationError::new(
                GenerationErrorKind::UnsupportedParameter,
                "Thinking Budget must be a non-negative integer",
            )
            .with_detail(error.to_string())
        })?;
        generation_config.insert(
            "thinkingConfig".into(),
            json!({"thinkingBudget": budget, "includeThoughts": true}),
        );
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
    use crate::{
        domain::ChatMessage,
        domain::{GenerationConfig, Model, ProviderKind},
    };

    fn request() -> GenerationRequest {
        let provider = Provider::new("Gemini", ProviderKind::Gemini);
        let mut model = Model::new(&provider.id, "gemini-test", "Gemini Test");
        model.capabilities = ProviderKind::Gemini.default_capabilities();
        GenerationRequest {
            provider,
            model,
            system_prompt: "Be concise".into(),
            config: GenerationConfig {
                temperature: Some(0.2),
                top_p: Some(0.8),
                top_k: Some(40),
                max_output_tokens: Some(512),
                frequency_penalty: Some(0.1),
                presence_penalty: Some(0.2),
                seed: Some(7),
                stop_sequences: vec!["stop".into()],
                thinking_budget: Some(2048),
                extra: Map::from_iter([(
                    "generationConfig".into(),
                    json!({"topK": 999, "responseMimeType": "text/plain", "seed": 999}),
                )]),
            },
            messages: vec![ChatMessage {
                role: crate::domain::MessageRole::User,
                content: "Hello".into(),
            }],
        }
    }

    #[test]
    fn request_parameters_follow_gemini_capabilities() {
        let request = request();
        let parameters = additional_parameters(&request).unwrap();
        let generation = parameters["generationConfig"].as_object().unwrap();
        let sdk_request = sdk_request(&request, parameters.clone()).unwrap();

        assert_eq!(sdk_request.temperature, Some(0.2));
        assert_eq!(sdk_request.max_tokens, Some(512));
        assert_eq!(generation["topP"], 0.8);
        assert_eq!(generation["topK"], 40);
        assert_eq!(generation["frequencyPenalty"], 0.1);
        assert_eq!(generation["presencePenalty"], 0.2);
        assert_eq!(generation["stopSequences"], json!(["stop"]));
        assert_eq!(generation["thinkingConfig"]["thinkingBudget"], 2048);
        assert_eq!(generation["responseMimeType"], "text/plain");
        assert!(generation.get("seed").is_none());
        assert_eq!(sdk_request.preamble.as_deref(), Some("Be concise"));
    }

    #[test]
    fn base_url_is_normalized_for_rig() {
        let mut provider = Provider::new("Gemini", ProviderKind::Gemini);
        provider.endpoint =
            "http://localhost:8080/v1beta/models/gemini:streamGenerateContent".into();
        assert_eq!(sdk_base_url(&provider).unwrap(), "http://localhost:8080");
    }

    #[tokio::test]
    async fn rig_stream_handles_fragmented_candidates_thinking_usage_and_request_body() {
        use crate::providers::test_support::{fragmented, request_json, server};

        let fixture = include_str!("../../tests/fixtures/gemini_success.sse");
        let (endpoint, captured) =
            server("200 OK", "text/event-stream", fragmented(fixture, 11)).await;
        let mut request = request();
        request.provider.endpoint = endpoint;
        request
            .provider
            .headers
            .insert("X-Test".into(), "value".into());
        let (sender, receiver) = async_channel::unbounded();

        stream(request, &sender, CancellationToken::new())
            .await
            .unwrap();
        let raw_request = captured.await.unwrap();
        let body = request_json(&raw_request);
        assert!(raw_request.to_lowercase().contains("x-test: value"));
        assert_eq!(body["systemInstruction"]["parts"][0]["text"], "Be concise");
        assert_eq!(body["generationConfig"]["temperature"], 0.2);
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 512);
        assert_eq!(body["generationConfig"]["topP"], 0.8);
        assert_eq!(body["generationConfig"]["topK"], 40);
        assert_eq!(
            body["generationConfig"]["thinkingConfig"]["thinkingBudget"],
            2048
        );
        assert!(body["generationConfig"].get("seed").is_none());

        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        assert_eq!(events.first(), Some(&GenerationEvent::Started));
        assert!(events.contains(&GenerationEvent::ThinkingDelta("Think".into())));
        assert!(events.contains(&GenerationEvent::TextDelta("Hello".into())));
        assert!(
            events.contains(&GenerationEvent::UsageUpdated(crate::domain::TokenUsage {
                input_tokens: Some(4),
                output_tokens: Some(3),
                estimated: false,
            }))
        );
        assert_eq!(events.last(), Some(&GenerationEvent::Completed));
    }

    #[tokio::test]
    async fn rig_stream_maps_provider_error_and_interrupted_fixture() {
        use crate::providers::test_support::{fragmented, server};
        use std::time::Duration;

        let error = include_str!("../../tests/fixtures/gemini_error.json");
        let (endpoint, _) = server(
            "429 Too Many Requests",
            "application/json",
            vec![(Duration::ZERO, error.into())],
        )
        .await;
        let mut failed = request();
        failed.provider.endpoint = endpoint;
        let (sender, _receiver) = async_channel::unbounded();
        assert_eq!(
            stream(failed, &sender, CancellationToken::new())
                .await
                .unwrap_err()
                .kind,
            GenerationErrorKind::RateLimited
        );

        let interrupted = include_str!("../../tests/fixtures/gemini_interrupted.sse");
        let (endpoint, _) = server("200 OK", "text/event-stream", fragmented(interrupted, 5)).await;
        let mut failed = request();
        failed.provider.endpoint = endpoint;
        let (sender, _receiver) = async_channel::unbounded();
        assert_eq!(
            stream(failed, &sender, CancellationToken::new())
                .await
                .unwrap_err()
                .kind,
            GenerationErrorKind::StreamInterrupted
        );
    }

    #[tokio::test]
    async fn connection_test_uses_rig_models_endpoint() {
        use crate::providers::test_support::server;
        use std::time::Duration;

        let (endpoint, captured) = server(
            "200 OK",
            "application/json",
            vec![(Duration::ZERO, "{}".into())],
        )
        .await;
        let mut provider = Provider::new("Gemini", ProviderKind::Gemini);
        provider.endpoint = endpoint;
        provider.api_key = "test-key".into();
        test_connection(&provider).await.unwrap();

        let request = captured.await.unwrap();
        assert!(request.starts_with("GET /v1beta/models?key=test-key "));
    }
}
