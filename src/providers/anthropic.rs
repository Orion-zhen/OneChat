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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        GenerationConfig, Model, ProviderKind, ToolDefinition, message_tool_calls,
    };

    fn request() -> GenerationRequest {
        let provider = Provider::new("Anthropic", ProviderKind::Anthropic);
        let mut model = Model::new(&provider.id, "claude-test", "Claude Test");
        model.capabilities = ProviderKind::Anthropic.default_capabilities();
        GenerationRequest {
            provider,
            model,
            system_prompt: "Be concise".into(),
            config: GenerationConfig {
                temperature: Some(0.2),
                top_p: Some(0.8),
                top_k: Some(40),
                max_output_tokens: Some(512),
                frequency_penalty: Some(1.0),
                stop_sequences: vec!["stop".into()],
                thinking_budget: Some(2048),
                extra: Map::from_iter([
                    ("top_k".into(), json!(999)),
                    ("tools".into(), json!([{"name": "override"}])),
                    ("tool_choice".into(), json!({"type": "any"})),
                ]),
                ..GenerationConfig::default()
            },
            messages: vec![Message::user("Hello")],
            tools: Vec::new(),
        }
    }

    #[test]
    fn request_parameters_follow_anthropic_capabilities() {
        let request = request();
        let parameters = additional_parameters(&request);
        let sdk_request = sdk_request(&request, parameters.clone()).unwrap();

        assert_eq!(sdk_request.temperature, Some(0.2));
        assert_eq!(sdk_request.max_tokens, Some(512));
        assert_eq!(parameters["top_p"], 0.8);
        assert_eq!(parameters["top_k"], 40);
        assert_eq!(parameters["stop_sequences"], json!(["stop"]));
        assert_eq!(parameters["thinking"]["budget_tokens"], 2048);
        assert!(parameters.get("frequency_penalty").is_none());
        assert!(parameters.get("tools").is_none());
        assert!(parameters.get("tool_choice").is_none());
        assert_eq!(sdk_request.preamble.as_deref(), Some("Be concise"));
    }

    #[test]
    fn base_url_is_normalized_for_rig() {
        let mut provider = Provider::new("Anthropic", ProviderKind::Anthropic);
        provider.endpoint = "http://localhost:8080/v1/messages".into();
        assert_eq!(sdk_base_url(&provider).unwrap(), "http://localhost:8080");
    }

    #[tokio::test]
    async fn rig_stream_handles_fragmented_text_thinking_usage_and_request_body() {
        use crate::providers::test_support::{fragmented, request_json, server};

        let fixture = include_str!("../../tests/fixtures/anthropic_success.sse");
        let (endpoint, captured) =
            server("200 OK", "text/event-stream", fragmented(fixture, 13)).await;
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
        assert_eq!(body["model"], "claude-test");
        assert_eq!(body["system"][0]["text"], "Be concise");
        assert_eq!(body["top_p"], 0.8);
        assert_eq!(body["top_k"], 40);
        assert_eq!(body["thinking"]["budget_tokens"], 2048);
        assert!(body.get("frequency_penalty").is_none());

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
        assert!(!events.contains(&GenerationEvent::Completed));
    }

    #[tokio::test]
    async fn stream_returns_tool_calls_and_sends_definitions() {
        use crate::providers::test_support::{fragmented, request_json, server};

        let fixture = include_str!("../../tests/fixtures/anthropic_tool_call.sse");
        let (endpoint, captured) =
            server("200 OK", "text/event-stream", fragmented(fixture, 13)).await;
        let mut request = request();
        request.provider.endpoint = endpoint;
        request.tools.push(ToolDefinition {
            name: "fixture__environment".into(),
            description: "Read the environment".into(),
            parameters: json!({"type": "object", "properties": {}}),
        });
        let (sender, _receiver) = async_channel::unbounded();

        let message = stream(request, &sender, CancellationToken::new())
            .await
            .unwrap();
        let calls = message_tool_calls(&message);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "toolu_1");
        assert_eq!(calls[0].function.name, "fixture__environment");
        assert_eq!(calls[0].function.arguments, json!({}));

        let body = request_json(&captured.await.unwrap());
        assert_eq!(body["tools"][0]["name"], "fixture__environment");
        assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
    }

    #[tokio::test]
    async fn rig_stream_maps_provider_error_and_interrupted_fixture() {
        use crate::providers::test_support::{fragmented, server};
        use std::time::Duration;

        let error = include_str!("../../tests/fixtures/anthropic_error.json");
        let (endpoint, _) = server(
            "400 Bad Request",
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
            GenerationErrorKind::UnsupportedParameter
        );

        let interrupted = include_str!("../../tests/fixtures/anthropic_interrupted.sse");
        let (endpoint, _) = server("200 OK", "text/event-stream", fragmented(interrupted, 7)).await;
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
    async fn connection_test_and_cancellation_use_rig() {
        use crate::providers::test_support::server;
        use std::time::Duration;

        let (endpoint, _) = server(
            "200 OK",
            "application/json",
            vec![(Duration::ZERO, "{}".into())],
        )
        .await;
        let mut provider = Provider::new("Anthropic", ProviderKind::Anthropic);
        provider.endpoint = endpoint;
        test_connection(&provider).await.unwrap();

        let fixture = include_str!("../../tests/fixtures/anthropic_success.sse");
        let (endpoint, _) = server(
            "200 OK",
            "text/event-stream",
            vec![(Duration::from_secs(2), fixture.into())],
        )
        .await;
        let mut request = request();
        request.provider.endpoint = endpoint;
        let (sender, _receiver) = async_channel::unbounded();
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move { stream(request, &sender, task_cancellation).await });
        tokio::time::sleep(Duration::from_millis(30)).await;
        cancellation.cancel();

        assert_eq!(
            task.await.unwrap().unwrap_err().kind,
            GenerationErrorKind::UserCancelled
        );
    }
}
