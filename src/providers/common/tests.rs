use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use rig_core::{
    completion::{CompletionError, CompletionResponse, Usage},
    streaming::{RawStreamingChoice, StreamFinal, StreamingResult},
};

use super::*;
use crate::domain::{GenerationConfig, Model};

#[derive(Clone, Copy)]
enum StreamScenario {
    Final,
    Interrupted,
    Pending,
}

#[derive(Clone)]
struct MockModel {
    scenario: StreamScenario,
    stream_calls: Arc<AtomicUsize>,
}

impl MockModel {
    fn new(scenario: StreamScenario) -> Self {
        Self {
            scenario,
            stream_calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl CompletionModel for MockModel {
    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> Result<CompletionResponse, CompletionError> {
        panic!("completion is not used by stream_model tests")
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> Result<StreamingCompletionResponse, CompletionError> {
        self.stream_calls.fetch_add(1, Ordering::SeqCst);
        let items = match self.scenario {
            StreamScenario::Final => vec![Ok(RawStreamingChoice::FinalResponse(StreamFinal::new(
                "mock",
                Usage::default(),
            )))],
            StreamScenario::Interrupted => vec![
                Ok(RawStreamingChoice::Message("partial".into())),
                Err(CompletionError::ProviderError("stream failed".into())),
            ],
            StreamScenario::Pending => {
                return std::future::pending().await;
            }
        };
        let stream: StreamingResult = Box::pin(futures_util::stream::iter(items));
        Ok(StreamingCompletionResponse::stream("mock", stream))
    }
}

fn request() -> CompletionRequest {
    CompletionRequest {
        model: Some("model".into()),
        preamble: None,
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
    let model = Model::new(&provider.id, "model", "Model");
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
    assert!(sdk.preamble.is_none());
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
    let model = MockModel::new(StreamScenario::Final);
    let calls = model.stream_calls.clone();
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
    let model = MockModel::new(StreamScenario::Pending);
    let calls = model.stream_calls.clone();
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
    let missing_usage = stream_model(
        MockModel::new(StreamScenario::Final),
        request(),
        &events,
        CancellationToken::new(),
        true,
    )
    .await
    .unwrap_err();
    assert_eq!(missing_usage.kind, GenerationErrorKind::StreamInterrupted);

    let interrupted = stream_model(
        MockModel::new(StreamScenario::Interrupted),
        request(),
        &events,
        CancellationToken::new(),
        false,
    )
    .await
    .unwrap_err();
    assert_eq!(interrupted.kind, GenerationErrorKind::StreamInterrupted);
}
