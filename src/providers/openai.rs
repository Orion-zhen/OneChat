use async_channel::Sender;
use futures_util::StreamExt;
use rig_core::{
    client::{CompletionClient, VerifyClient},
    completion::{CompletionModel, CompletionRequest, GetTokenUsage},
    providers::openai as rig_openai,
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

type OpenAiClient = rig_openai::Client<reqwest::Client>;

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
    let model_id = request.model.remote_id.clone();
    let sdk_request = sdk_request(&request, additional_parameters(&request))?;
    match request.provider.kind {
        crate::domain::ProviderKind::OpenAi => {
            stream_model(
                client.completion_model(model_id),
                sdk_request,
                events,
                cancellation,
            )
            .await
        }
        crate::domain::ProviderKind::OpenAiCompatible => {
            stream_model(
                client.completions_api().completion_model(model_id),
                sdk_request,
                events,
                cancellation,
            )
            .await
        }
        _ => unreachable!("openai::stream called for a non-OpenAI provider"),
    }
}

async fn stream_model<M: CompletionModel>(
    model: M,
    request: CompletionRequest,
    events: &Sender<GenerationEvent>,
    cancellation: CancellationToken,
) -> Result<(), GenerationError> {
    let mut response = tokio::select! {
        _ = cancellation.cancelled() => return Err(GenerationError::cancelled()),
        response = model.stream(request) => response.map_err(|error| sdk_completion_error(error, false))?,
    };

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
            }
            _ => {}
        }
    }

    if !saw_final || (had_output && !saw_usage) {
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

fn additional_parameters(request: &GenerationRequest) -> Map<String, Value> {
    let capabilities = &request.model.capabilities;
    let config = &request.config;
    let mut parameters = config.extra.clone();
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
            "thinking_budget",
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
    insert_optional(
        &mut parameters,
        "thinking_budget",
        capabilities
            .thinking_budget
            .then_some(config.thinking_budget)
            .flatten(),
    );
    parameters
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
mod tests {
    use super::*;
    use crate::{
        domain::ChatMessage,
        domain::{GenerationConfig, Model, ProviderKind},
    };

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
                extra: Map::from_iter([
                    (
                        "reasoning".into(),
                        json!({ "effort": "high", "summary": "auto" }),
                    ),
                    ("top_k".into(), json!(999)),
                ]),
                ..GenerationConfig::default()
            },
            messages: vec![ChatMessage {
                role: crate::domain::MessageRole::User,
                content: "Hello".into(),
            }],
        }
    }

    #[test]
    fn request_parameters_are_filtered_before_rig_serializes_them() {
        let request = request();
        let parameters = additional_parameters(&request);
        let sdk_request = sdk_request(&request, parameters.clone()).unwrap();

        assert_eq!(sdk_request.temperature, Some(0.2));
        assert_eq!(parameters["seed"], 7);
        assert!(parameters.get("top_k").is_none());
        assert_eq!(parameters["reasoning"]["effort"], "high");
        assert_eq!(sdk_request.preamble.as_deref(), Some("Be concise"));
    }

    #[test]
    fn empty_system_prompt_is_not_sent() {
        let mut request = request();
        request.system_prompt = "   ".into();

        let sdk_request = sdk_request(&request, additional_parameters(&request)).unwrap();

        assert!(sdk_request.preamble.is_none());
    }

    #[test]
    fn base_url_accepts_a_complete_custom_path() {
        let mut provider = Provider::new("Local", ProviderKind::OpenAiCompatible);
        provider.endpoint = "http://localhost:8080/v1/chat/completions".into();
        assert_eq!(sdk_base_url(&provider).unwrap(), "http://localhost:8080/v1");
        provider.endpoint = "http://localhost:8080/v1/models".into();
        assert_eq!(sdk_base_url(&provider).unwrap(), "http://localhost:8080/v1");

        provider.kind = ProviderKind::OpenAi;
        provider.endpoint = "http://localhost:8080/v1/responses".into();
        assert_eq!(sdk_base_url(&provider).unwrap(), "http://localhost:8080/v1");
    }

    #[tokio::test]
    async fn rig_stream_handles_fragmented_openai_sse_and_request_body() {
        use crate::providers::test_support::{fragmented, request_json, server};

        let fixture = include_str!("../../tests/fixtures/openai_success.sse");
        let (endpoint, captured) =
            server("200 OK", "text/event-stream", fragmented(fixture, 9)).await;
        let mut request = request();
        request.provider.endpoint = format!("{endpoint}/v1");
        let (sender, receiver) = async_channel::unbounded();

        stream(request, &sender, CancellationToken::new())
            .await
            .unwrap();
        let captured = captured.await.unwrap();
        assert!(captured.starts_with("POST /v1/responses "));
        let body = request_json(&captured);
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["instructions"], "Be concise");
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["temperature"], 0.2);
        assert_eq!(body["reasoning"]["effort"], "high");
        assert_eq!(body["reasoning"]["summary"], "auto");
        assert!(body.get("seed").is_none());
        assert!(body.get("top_k").is_none());
        assert_eq!(body["stream"], true);

        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        assert!(events.contains(&GenerationEvent::ThinkingDelta("Think".into())));
        assert!(events.contains(&GenerationEvent::TextDelta("Hi".into())));
        assert!(
            events.contains(&GenerationEvent::UsageUpdated(crate::domain::TokenUsage {
                input_tokens: Some(3),
                output_tokens: Some(2),
                estimated: false,
            }))
        );
        assert_eq!(events.last(), Some(&GenerationEvent::Completed));
    }

    #[tokio::test]
    async fn openai_compatible_keeps_chat_completions_api() {
        use crate::providers::test_support::{fragmented, request_json, server};

        let fixture = include_str!("../../tests/fixtures/openai_chat_completions_success.sse");
        let (endpoint, captured) =
            server("200 OK", "text/event-stream", fragmented(fixture, 9)).await;
        let mut request = request();
        request.provider.kind = ProviderKind::OpenAiCompatible;
        request.provider.endpoint = format!("{endpoint}/v1");
        request.config.extra = Map::from_iter([
            ("reasoning_effort".into(), json!("high")),
            ("top_k".into(), json!(999)),
        ]);
        let (sender, receiver) = async_channel::unbounded();

        stream(request, &sender, CancellationToken::new())
            .await
            .unwrap();
        let captured = captured.await.unwrap();
        assert!(captured.starts_with("POST /v1/chat/completions "));
        let body = request_json(&captured);
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["seed"], 7);
        assert_eq!(body["reasoning_effort"], "high");
        assert_eq!(body["stream_options"]["include_usage"], true);

        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        assert!(events.contains(&GenerationEvent::ThinkingDelta("Think".into())));
        assert!(events.contains(&GenerationEvent::TextDelta("Hi".into())));
        assert_eq!(events.last(), Some(&GenerationEvent::Completed));
    }

    #[tokio::test]
    async fn rig_stream_maps_stream_http_errors_and_early_eof() {
        use crate::providers::test_support::{fragmented, server};
        use std::time::Duration;

        let fixture = include_str!("../../tests/fixtures/openai_error.sse");
        let (endpoint, _) = server("200 OK", "text/event-stream", fragmented(fixture, 5)).await;
        let mut failed = request();
        failed.provider.endpoint = format!("{endpoint}/v1");
        let (sender, _receiver) = async_channel::unbounded();
        assert_eq!(
            stream(failed, &sender, CancellationToken::new())
                .await
                .unwrap_err()
                .kind,
            GenerationErrorKind::UnsupportedParameter
        );

        let (endpoint, _) = server(
            "401 Unauthorized",
            "application/json",
            vec![(
                Duration::ZERO,
                "{\"error\":{\"message\":\"bad key\"}}".into(),
            )],
        )
        .await;
        let mut failed = request();
        failed.provider.endpoint = format!("{endpoint}/v1");
        let (sender, _receiver) = async_channel::unbounded();
        assert_eq!(
            stream(failed, &sender, CancellationToken::new())
                .await
                .unwrap_err()
                .kind,
            GenerationErrorKind::Authentication
        );

        let fixture = include_str!("../../tests/fixtures/openai_interrupted.sse");
        let (endpoint, _) = server("200 OK", "text/event-stream", fragmented(fixture, 4)).await;
        let mut failed = request();
        failed.provider.endpoint = format!("{endpoint}/v1");
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
        let mut provider = Provider::new("Local", ProviderKind::OpenAiCompatible);
        provider.endpoint = format!("{endpoint}/v1");
        test_connection(&provider).await.unwrap();
        assert!(captured.await.unwrap().starts_with("GET /v1/models "));
    }
}
