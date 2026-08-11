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
        GenerationEvent::ThinkingDelta {
            provider_id: Some("reasoning-1".into()),
            delta: "working".into(),
        },
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

    assert!(text.finished_reasoning_id.is_some());
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

#[test]
fn interleaved_reasoning_output_and_tools_keep_stream_order() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let mut response = AssistantResponse::new(&model, &provider);
    response.status = MessageStatus::Streaming;
    let mut request = RequestInfo::new("conversation", "turn", &response.id);

    let apply = |event, elapsed_ms, response: &mut AssistantResponse, request: &mut RequestInfo| {
        apply_event(event, response, request, Duration::from_millis(elapsed_ms))
    };
    apply(
        GenerationEvent::ThinkingDelta {
            provider_id: Some("reasoning-a".into()),
            delta: "A".into(),
        },
        10,
        &mut response,
        &mut request,
    );
    apply(
        GenerationEvent::ToolCallObserved {
            internal_call_id: "internal-a".into(),
            provider_tool_call_id: "call-a".into(),
        },
        20,
        &mut response,
        &mut request,
    );
    let tool_a = ToolExecution::new("call-a", None, "server", "tool-a", Value::Null);
    apply(
        GenerationEvent::ToolExecutionUpdated(Box::new(tool_a.clone())),
        25,
        &mut response,
        &mut request,
    );
    apply(
        GenerationEvent::ToolExecutionUpdated(Box::new(tool_a)),
        26,
        &mut response,
        &mut request,
    );
    apply(
        GenerationEvent::ThinkingDelta {
            provider_id: Some("reasoning-b".into()),
            delta: "B".into(),
        },
        30,
        &mut response,
        &mut request,
    );
    apply(
        GenerationEvent::TextDelta("intermediate".into()),
        40,
        &mut response,
        &mut request,
    );
    apply(
        GenerationEvent::ToolCallObserved {
            internal_call_id: "internal-b".into(),
            provider_tool_call_id: "call-b".into(),
        },
        50,
        &mut response,
        &mut request,
    );
    let tool_b = ToolExecution::new("call-b", None, "server", "tool-b", Value::Null);
    apply(
        GenerationEvent::ToolExecutionUpdated(Box::new(tool_b)),
        55,
        &mut response,
        &mut request,
    );
    apply(
        GenerationEvent::ThinkingDelta {
            provider_id: Some("reasoning-c".into()),
            delta: "C".into(),
        },
        60,
        &mut response,
        &mut request,
    );
    apply(
        GenerationEvent::TextDelta("final".into()),
        70,
        &mut response,
        &mut request,
    );

    assert_eq!(response.thinking, "ABC");
    assert_eq!(response.content, "intermediatefinal");
    assert_eq!(response.tool_executions.len(), 2);
    assert_eq!(response.blocks.len(), 7);
    assert!(matches!(
        &response.blocks[0],
        AssistantBlock::Reasoning { content, duration_ms: Some(10), .. } if content == "A"
    ));
    assert!(matches!(
        &response.blocks[1],
        AssistantBlock::ToolCall {
            execution_id: Some(_),
            ..
        }
    ));
    assert!(matches!(
        &response.blocks[2],
        AssistantBlock::Reasoning { content, duration_ms: Some(10), .. } if content == "B"
    ));
    assert!(
        matches!(&response.blocks[3], AssistantBlock::Output { content, .. } if content == "intermediate")
    );
    assert!(matches!(
        &response.blocks[4],
        AssistantBlock::ToolCall {
            execution_id: Some(_),
            ..
        }
    ));
    assert!(matches!(
        &response.blocks[5],
        AssistantBlock::Reasoning { content, duration_ms: Some(10), .. } if content == "C"
    ));
    assert!(
        matches!(&response.blocks[6], AssistantBlock::Output { content, .. } if content == "final")
    );
}

#[test]
fn editing_outputs_preserves_positions_and_serialization() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let mut response = AssistantResponse::new(&model, &provider);
    response.content = "firstsecond".into();
    response.blocks = vec![
        AssistantBlock::Output {
            id: "output-1".into(),
            content: "first".into(),
        },
        AssistantBlock::Output {
            id: "output-2".into(),
            content: "second".into(),
        },
    ];
    response.transcript = vec![Message::assistant("first"), Message::assistant("second")];

    response.replace_outputs(&[
        ("output-1".into(), String::new()),
        ("output-2".into(), "revised".into()),
    ]);

    assert_eq!(response.content, "revised");
    assert_eq!(response.blocks.len(), 1);
    assert!(matches!(
        &response.blocks[0],
        AssistantBlock::Output { id, content } if id == "output-2" && content == "revised"
    ));
    let transcript_texts = response
        .transcript
        .iter()
        .flat_map(|message| match message {
            Message::Assistant { content, .. } => content
                .iter()
                .filter_map(|item| match item {
                    rig_core::completion::AssistantContent::Text(text) => Some(text.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    assert_eq!(transcript_texts, vec!["", "revised"]);

    let restored: AssistantResponse =
        serde_json::from_str(&serde_json::to_string(&response).unwrap()).unwrap();
    assert_eq!(restored.blocks, response.blocks);
    assert_eq!(restored.content, response.content);
}
