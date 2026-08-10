use super::*;

#[test]
fn streaming_events_produce_completed_and_cancelled_states() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let mut response = AssistantResponse::new(&model, &provider);
    response.status = MessageStatus::Streaming;
    let mut request = RequestInfo::new("conversation", "turn", &response.id);

    apply_event(
        GenerationEvent::Started,
        &mut response,
        &mut request,
        Duration::ZERO,
    );
    apply_event(
        GenerationEvent::ThinkingDelta("working".into()),
        &mut response,
        &mut request,
        Duration::from_millis(10),
    );
    let text = apply_event(
        GenerationEvent::TextDelta("answer".into()),
        &mut response,
        &mut request,
        Duration::from_millis(30),
    );
    let completed = apply_event(
        GenerationEvent::Completed,
        &mut response,
        &mut request,
        Duration::from_millis(50),
    );

    assert!(text.thinking_finished);
    assert!(completed.terminal);
    assert_eq!(response.thinking, "working");
    assert_eq!(response.content, "answer");
    assert_eq!(response.status, MessageStatus::Completed);
    assert_eq!(request.status, RequestStatus::Completed);
    assert_eq!(request.ttft_ms, Some(10));
    assert_eq!(request.thinking_duration_ms, Some(30));
    assert_eq!(request.duration_ms, Some(50));
    assert!(request.usage.output_tokens.is_some());

    let cancelled = apply_event(
        GenerationEvent::Failed(GenerationError::cancelled()),
        &mut response,
        &mut request,
        Duration::from_millis(60),
    );
    assert!(cancelled.terminal);
    assert_eq!(response.status, MessageStatus::Stopped);
    assert_eq!(request.status, RequestStatus::Stopped);
    assert!(request.error.is_none());
}

#[test]
fn provider_usage_keeps_the_last_step_separate_from_cumulative_usage() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let mut response = AssistantResponse::new(&model, &provider);
    let mut request = RequestInfo::new("conversation", "turn", &response.id);
    request.usage.input_tokens = Some(90);
    request.usage.estimated = true;

    apply_event(
        GenerationEvent::StepStarted {
            estimated_input_tokens: 100,
        },
        &mut response,
        &mut request,
        Duration::ZERO,
    );
    apply_event(
        GenerationEvent::UsageUpdated(TokenUsage {
            input_tokens: Some(120),
            output_tokens: Some(10),
            estimated: false,
        }),
        &mut response,
        &mut request,
        Duration::ZERO,
    );
    apply_event(
        GenerationEvent::StepStarted {
            estimated_input_tokens: 150,
        },
        &mut response,
        &mut request,
        Duration::ZERO,
    );
    apply_event(
        GenerationEvent::UsageUpdated(TokenUsage {
            input_tokens: Some(180),
            output_tokens: Some(20),
            estimated: false,
        }),
        &mut response,
        &mut request,
        Duration::ZERO,
    );

    assert_eq!(request.usage.input_tokens, Some(300));
    assert_eq!(request.usage.output_tokens, Some(30));
    assert_eq!(request.last_step_input_tokens, Some(180));
    assert_eq!(request.last_step_estimated_input_tokens, Some(150));
}
