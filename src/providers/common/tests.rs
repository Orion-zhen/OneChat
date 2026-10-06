use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use rig_core::{
    completion::{CompletionResponse, Usage},
    driver::{Exchange, Opened, Opening},
    error::ProviderError,
    providers::openai::wire::{Chat, OpenAIConfig},
    streaming::{Relayed, StreamEvents, Transcript},
};

use super::*;
use crate::domain::{GenerationConfig, Model as DomainModel};

#[derive(Clone, Default)]
struct PendingTransport {
    stream_calls: Arc<AtomicUsize>,
}

impl Transport<Chat> for PendingTransport {
    fn send(
        &self,
        _payload: <Chat as Wire>::Payload,
        _exchange: Exchange,
    ) -> Opening<<Chat as Wire>::Frame> {
        let calls = self.stream_calls.clone();
        Opening::new(async move {
            calls.fetch_add(1, Ordering::SeqCst);
            std::future::pending::<Result<Opened<<Chat as Wire>::Frame>, ProviderError>>().await
        })
    }
}

fn pending_model() -> Model<Chat, PendingTransport> {
    Model::new(
        OpenAIConfig::new("test").chat("model"),
        PendingTransport::default(),
    )
}

fn relayed(items: Vec<Result<Relayed, rig_core::error::ErrorReport>>) -> CompletionStream {
    let stream: StreamEvents = Box::pin(futures_util::stream::iter(items));
    CompletionStream::relay("mock", stream)
}

fn request() -> CompletionRequest {
    CompletionRequest {
        model: Some("model".into()),
        chat_history: vec![Message::user("Hello")],
        documents: Vec::new(),
        tools: Vec::new(),
        temperature: None,
        max_tokens: None,
        tool_choice: None,
        additional_params: None,
        output_schema: None,
        record_telemetry_content: false,
    }
}

fn generation_request(system_prompt: &str, messages: Vec<Message>) -> GenerationRequest {
    let provider = Provider::new("Provider", ProviderKind::OpenAi);
    let model = DomainModel::new(&provider.id, "model", "Model", provider.kind);
    GenerationRequest {
        provider,
        model,
        system_prompt: system_prompt.into(),
        config: GenerationConfig::default(),
        messages,
        audio_duration_ms: 0,
        tools: Vec::new(),
    }
}

#[test]
fn sdk_request_uses_system_messages_and_validates_empty_content() {
    let request = generation_request("System prompt", vec![Message::user("Hello")]);
    let sdk = sdk_request(&request, Map::new()).unwrap();
    assert!(matches!(
        sdk.chat_history.first(),
        Some(Message::System { content }) if content == "System prompt"
    ));

    let request = generation_request(
        "",
        vec![Message::User {
            content: Vec::new(),
        }],
    );
    let error = sdk_request(&request, Map::new()).unwrap_err();
    assert_eq!(error.kind, GenerationErrorKind::UnsupportedParameter);
}

#[tokio::test]
async fn stream_model_does_not_send_an_already_cancelled_request() {
    let model = pending_model();
    let calls = model.transport.stream_calls.clone();
    let (events, _event_rx) = async_channel::unbounded();
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    let error = stream_model(model, request(), &events, cancellation, false)
        .await
        .unwrap_err();

    assert_eq!(error.kind, GenerationErrorKind::UserCancelled);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn stream_model_cancels_while_waiting_for_the_stream() {
    let model = pending_model();
    let calls = model.transport.stream_calls.clone();
    let (events, _event_rx) = async_channel::unbounded();
    let cancellation = CancellationToken::new();
    let task_cancellation = cancellation.clone();
    let task = tokio::spawn(async move {
        stream_model(model, request(), &events, task_cancellation, false).await
    });
    while calls.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }

    cancellation.cancel();
    let error = task.await.unwrap().unwrap_err();

    assert_eq!(error.kind, GenerationErrorKind::UserCancelled);
}

#[test]
fn normalized_finish_reasons_have_one_provider_independent_policy() {
    for reason in [
        None,
        Some(FinishReason::Stop),
        Some(FinishReason::Length),
        Some(FinishReason::ToolCalls),
    ] {
        assert!(validate_finish_reason(reason.as_ref()).is_ok());
    }

    let filtered = validate_finish_reason(Some(&FinishReason::ContentFilter)).unwrap_err();
    assert_eq!(filtered.kind, GenerationErrorKind::Unknown);

    let other = validate_finish_reason(Some(&FinishReason::Other("blocked".into()))).unwrap_err();
    assert_eq!(other.detail.as_deref(), Some("finish_reason=blocked"));
}

#[tokio::test]
async fn stream_model_enforces_usage_and_marks_streaming_errors() {
    let (events, _event_rx) = async_channel::unbounded();
    let missing_usage = consume_stream(
        relayed(vec![Ok(Relayed::Done(Box::new(CompletionResponse::new(
            Vec::new(),
            Usage::default(),
            "mock",
            Value::Null,
        ))))]),
        &events,
        CancellationToken::new(),
        true,
    )
    .await
    .unwrap_err();
    assert_eq!(missing_usage.kind, GenerationErrorKind::StreamInterrupted);

    let transcript = Transcript::parse_prefix(json!([
        {"item": "event", "value": {"event": "start", "part": 0, "kind": "text"}},
        {"item": "event", "value": {"event": "text", "part": 0, "text": "partial"}}
    ]))
    .unwrap();
    let mut items = transcript
        .into_items()
        .into_iter()
        .map(|item| Ok(Relayed::Item(item)))
        .collect::<Vec<_>>();
    items.push(Err(ProviderError::Provider("stream failed".into()).report()));
    let interrupted = consume_stream(relayed(items), &events, CancellationToken::new(), false)
        .await
        .unwrap_err();
    assert_eq!(interrupted.kind, GenerationErrorKind::StreamInterrupted);
}

#[tokio::test]
async fn streamed_parts_preserve_order_without_repeating_reasoning() {
    use rig_core::message::Reasoning;

    let reasoning = AssistantContent::Reasoning(Reasoning::new("thinking").sealed("openai"));
    let tool = AssistantContent::tool_call("call", "lookup".try_into().unwrap(), json!({}));
    let text = AssistantContent::text("answer");
    let transcript = Transcript::parse(json!([
        {"item": "event", "value": {"event": "start", "part": 0, "kind": "reasoning"}},
        {"item": "event", "value": {"event": "reasoning", "part": 0, "text": "thinking"}},
        {"item": "event", "value": {"event": "end", "part": 0, "content": reasoning}},
        {"item": "event", "value": {"event": "start", "part": 1, "kind": "tool_call"}},
        {"item": "event", "value": {"event": "arguments", "part": 1, "json": "{}"}},
        {"item": "event", "value": {"event": "end", "part": 1, "content": tool}},
        {"item": "event", "value": {"event": "start", "part": 2, "kind": "text"}},
        {"item": "event", "value": {"event": "text", "part": 2, "text": "answer"}},
        {"item": "event", "value": {"event": "end", "part": 2, "content": text}}
    ]))
    .unwrap();
    let choice = vec![reasoning, tool, text];
    let usage = Usage {
        input_tokens: Some(10),
        output_tokens: Some(0),
        ..Default::default()
    };
    let mut response = CompletionResponse::new(choice.clone(), usage, "mock", Value::Null);
    response.message_id = Some("message".into());
    let mut items = transcript
        .into_items()
        .into_iter()
        .map(|item| Ok(Relayed::Item(item)))
        .collect::<Vec<_>>();
    items.push(Ok(Relayed::Done(Box::new(response))));
    let (events, receiver) = async_channel::unbounded();

    let message = consume_stream(relayed(items), &events, CancellationToken::new(), true)
        .await
        .unwrap();

    assert_eq!(
        message,
        Message::Assistant {
            id: Some("message".into()),
            content: choice
        }
    );
    let actual = std::iter::from_fn(|| receiver.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            GenerationEvent::Started,
            GenerationEvent::ThinkingDelta {
                provider_id: None,
                delta: "thinking".into()
            },
            GenerationEvent::ToolCallObserved {
                stream_call_id: "1".into(),
                call_id: None
            },
            GenerationEvent::ToolCallObserved {
                stream_call_id: "1".into(),
                call_id: None
            },
            GenerationEvent::ToolCallObserved {
                stream_call_id: "1".into(),
                call_id: Some("call".into())
            },
            GenerationEvent::TextDelta("answer".into()),
            GenerationEvent::UsageUpdated(TokenUsage {
                input_tokens: Some(10),
                output_tokens: Some(0),
                estimated: false
            }),
        ]
    );
}

#[tokio::test]
async fn a_stream_without_a_terminal_response_is_interrupted() {
    let (events, _receiver) = async_channel::unbounded();
    let error = consume_stream(
        relayed(Vec::new()),
        &events,
        CancellationToken::new(),
        false,
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, GenerationErrorKind::StreamInterrupted);
}
