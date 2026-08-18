use super::*;
use rig_core::{completion::AssistantContent, message::Reasoning};

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
fn continued_transcript_merges_into_the_existing_assistant_message() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let mut response = AssistantResponse::new(&model, &provider);
    response.content = "existing".into();
    response.transcript = vec![Message::assistant("existing")];
    response.prepare_continuation();
    let mut request = RequestInfo::new("conversation", "turn", &response.id);
    request.kind = RequestKind::Continue;

    apply_event(
        GenerationEvent::TextDelta(" continuation".into()),
        &mut response,
        &mut request,
        Duration::from_millis(10),
    );
    apply_event(
        GenerationEvent::TranscriptContinued(Box::new(Message::assistant(" continuation"))),
        &mut response,
        &mut request,
        Duration::from_millis(20),
    );

    assert_eq!(response.content, "existing continuation");
    assert_eq!(response.transcript.len(), 1);
    assert!(serialized_messages(&response.transcript)[0].contains("existing continuation"));
}

#[test]
fn continued_transcript_removes_a_replayed_assistant_prefill() {
    let existing = Message::Assistant {
        id: None,
        content: vec![
            AssistantContent::Reasoning(Reasoning::new("old reasoning")),
            AssistantContent::text("old answer"),
        ],
    };
    let replayed = Message::Assistant {
        id: None,
        content: vec![
            AssistantContent::Reasoning(Reasoning::new("old reasoning")),
            AssistantContent::text("old answer continued"),
        ],
    };
    let mut transcript = vec![existing];

    continue_last_assistant(&mut transcript, replayed);

    let Message::Assistant { content, .. } = &transcript[0] else {
        panic!("expected assistant transcript");
    };
    assert_eq!(content.len(), 2);
    assert!(matches!(
        content.first(),
        Some(AssistantContent::Reasoning(reasoning)) if reasoning.display_text() == "old reasoning"
    ));
    assert!(matches!(
        content.last(),
        Some(AssistantContent::Text(text)) if text.text == "old answer continued"
    ));
}

#[test]
fn continued_transcript_preserves_a_suffix_only_response() {
    let mut transcript = vec![Message::assistant("old answer")];

    continue_last_assistant(&mut transcript, Message::assistant(" continued"));

    let Message::Assistant { content, .. } = &transcript[0] else {
        panic!("expected assistant transcript");
    };
    assert!(matches!(
        content.first(),
        Some(AssistantContent::Text(text)) if text.text == "old answer continued"
    ));
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
            stream_call_id: "internal-a".into(),
            call_id: None,
        },
        20,
        &mut response,
        &mut request,
    );
    apply(
        GenerationEvent::ToolCallObserved {
            stream_call_id: "internal-a".into(),
            call_id: Some("call-a".into()),
        },
        20,
        &mut response,
        &mut request,
    );
    let tool_a = ToolExecution::new("call-a", "server", "tool-a", Value::Null);
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
            stream_call_id: "internal-b".into(),
            call_id: Some("call-b".into()),
        },
        50,
        &mut response,
        &mut request,
    );
    let tool_b = ToolExecution::new("call-b", "server", "tool-b", Value::Null);
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
            call_id,
            execution_id: Some(_),
            ..
        } if call_id == "call-a"
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
fn editing_reasoning_updates_native_transcript_content() {
    use rig_core::{
        completion::AssistantContent,
        message::{Reasoning, ReasoningContent},
    };

    let provider = Provider::new("Local", ProviderKind::OpenAiCompatible);
    let model = Model::new(&provider.id, "qwen", "Qwen");
    let mut response = AssistantResponse::new(&model, &provider);
    response.thinking = "original reasoning".into();
    response.content = "answer".into();
    response.blocks = vec![
        AssistantBlock::Reasoning {
            id: "reasoning-block".into(),
            provider_id: Some("reasoning-provider".into()),
            content: "original reasoning".into(),
            started_after_ms: 0,
            duration_ms: Some(10),
        },
        AssistantBlock::Output {
            id: "output".into(),
            content: "answer".into(),
        },
    ];
    response.transcript = vec![Message::Assistant {
        id: None,
        content: vec![
            AssistantContent::Reasoning(
                Reasoning::new("original reasoning").with_id("reasoning-provider".into()),
            ),
            AssistantContent::text("answer"),
        ],
    }];

    response.replace_editable_text(
        &[("reasoning-block".into(), "edited reasoning".into())],
        &[("output".into(), "answer".into())],
    );

    assert_eq!(response.thinking, "edited reasoning");
    assert_eq!(response.content, "answer");
    assert!(matches!(
        &response.blocks[0],
        AssistantBlock::Reasoning { content, .. } if content == "edited reasoning"
    ));
    let Message::Assistant { content, .. } = &response.transcript[0] else {
        panic!("expected assistant transcript");
    };
    let Some(AssistantContent::Reasoning(reasoning)) = content.first() else {
        panic!("edited reasoning must remain native reasoning content");
    };
    assert_eq!(reasoning.id.as_deref(), Some("reasoning-provider"));
    assert!(matches!(
        reasoning.content.as_slice(),
        [ReasoningContent::Text { text, signature: None }] if text == "edited reasoning"
    ));
    assert!(matches!(
        content.iter().nth(1),
        Some(AssistantContent::Text(text)) if text.text == "answer"
    ));

    let wire: Vec<rig_core::providers::openai::completion::Message> =
        response.transcript[0].clone().try_into().unwrap();
    let wire = serde_json::to_value(&wire[0]).unwrap();
    assert_eq!(wire["reasoning_content"], "edited reasoning");
    assert_eq!(wire["content"][0]["text"], "answer");
}

#[test]
fn clearing_reasoning_removes_it_from_blocks_and_native_transcript() {
    use rig_core::{completion::AssistantContent, message::Reasoning};

    let provider = Provider::new("Local", ProviderKind::OpenAiCompatible);
    let model = Model::new(&provider.id, "qwen", "Qwen");
    let mut response = AssistantResponse::new(&model, &provider);
    response.thinking = "reasoning".into();
    response.content = "answer".into();
    response.blocks = vec![
        AssistantBlock::Reasoning {
            id: "reasoning".into(),
            provider_id: None,
            content: "reasoning".into(),
            started_after_ms: 0,
            duration_ms: Some(10),
        },
        AssistantBlock::Output {
            id: "output".into(),
            content: "answer".into(),
        },
    ];
    response.transcript = vec![Message::Assistant {
        id: None,
        content: vec![
            AssistantContent::Reasoning(Reasoning::new("reasoning")),
            AssistantContent::text("answer"),
        ],
    }];

    response.replace_editable_text(
        &[("reasoning".into(), "  ".into())],
        &[("output".into(), "answer".into())],
    );

    assert!(response.thinking.is_empty());
    assert!(
        response
            .blocks
            .iter()
            .all(|block| !matches!(block, AssistantBlock::Reasoning { .. }))
    );
    let Message::Assistant { content, .. } = &response.transcript[0] else {
        panic!("expected assistant transcript");
    };
    assert!(
        content
            .iter()
            .all(|item| !matches!(item, AssistantContent::Reasoning(_)))
    );
}

#[test]
fn editing_legacy_reasoning_creates_native_transcript() {
    use rig_core::completion::AssistantContent;

    let provider = Provider::new("Local", ProviderKind::OpenAiCompatible);
    let model = Model::new(&provider.id, "qwen", "Qwen");
    let mut response = AssistantResponse::new(&model, &provider);
    response.thinking = "original".into();
    response.content = "answer".into();

    response.replace_editable_text(
        &[(response.id.clone(), "edited".into())],
        &[(response.id.clone(), "answer".into())],
    );

    assert!(response.blocks.is_empty());
    assert_eq!(response.thinking, "edited");
    let Message::Assistant { content, .. } = &response.transcript[0] else {
        panic!("expected assistant transcript");
    };
    assert!(matches!(
        content.first(),
        Some(AssistantContent::Reasoning(reasoning)) if reasoning.display_text() == "edited"
    ));
    assert!(matches!(
        content.iter().nth(1),
        Some(AssistantContent::Text(text)) if text.text == "answer"
    ));
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
