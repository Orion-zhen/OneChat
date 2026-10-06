use super::*;
use rig_core::{
    completion::AssistantContent,
    message::{Reasoning, ReasoningContent, ToolResultContent, UserContent},
};

fn response() -> AssistantResponse {
    let provider = Provider::new("Test", ProviderKind::OpenAiCompatible);
    AssistantResponse::new(
        &Model::new(&provider.id, "model", "Model", provider.kind),
        &provider,
    )
}

#[test]
fn response_round_trip_stores_blocks_without_writable_text_summaries() {
    let mut response = response();
    response.append_reasoning(Some("native-reasoning".into()), "思考", 2);
    response.append_output("first", 10);
    let execution = ToolExecution::new("call", "server", "lookup", json!({"query": "test"}));
    response.upsert_tool_execution(execution, 12);
    response.append_output(" second", 20);
    response.transcript = vec![Message::assistant("first second")];

    let value = serde_json::to_value(&response).unwrap();
    assert!(value.get("content").is_none());
    assert!(value.get("thinking").is_none());
    let restored: AssistantResponse = serde_json::from_value(value).unwrap();
    assert_eq!(restored, response);
    assert_eq!(restored.output_text(), "first second");
    assert_eq!(restored.reasoning_blocks().next().unwrap().1, "思考");
    assert_eq!(restored.output_blocks().count(), 2);
    assert!(restored.has_reasoning());
    assert!(restored.is_usable_as_context());
}

#[test]
fn clearing_all_output_removes_context_eligibility_but_keeps_reasoning_and_tools() {
    let mut response = response();
    response.append_reasoning(None, "reasoning", 0);
    response.append_output("answer", 10);
    response.observe_tool_call("call".into(), None, 20);
    response.transcript = vec![Message::assistant("answer")];
    let output = response.output_blocks().next().unwrap().0.to_string();

    response
        .replace_editable_text(&[], &[(output, " \n\t ".into())])
        .unwrap();

    assert!(!response.has_output());
    assert!(!response.is_usable_as_context());
    assert_eq!(response.output_text(), "");
    assert_eq!(response.reasoning_blocks().next().unwrap().1, "reasoning");
    assert_eq!(response.blocks.len(), 2);
    assert!(matches!(
        response.blocks[1],
        AssistantBlock::ToolCall { .. }
    ));
}

#[test]
fn output_edits_leave_native_reasoning_signatures_tool_calls_and_results_unchanged() {
    let mut response = response();
    response.append_reasoning(Some("native-reasoning".into()), "reasoning", 0);
    response.append_output("before tool", 10);
    response.observe_tool_call("call".into(), None, 20);
    response.append_output("after tool", 30);
    let mut reasoning = Reasoning::new("reasoning").with_id("native-reasoning".into());
    reasoning.content = vec![ReasoningContent::Text {
        text: "reasoning".into(),
        signature: Some("signature".into()),
    }];
    let tool_call = AssistantContent::tool_call(
        "call",
        "lookup".try_into().unwrap(),
        json!({"query": "test"}),
    );
    let AssistantContent::ToolCall(call) = &tool_call else {
        panic!("expected tool call");
    };
    let tool_result = Message::User {
        content: vec![UserContent::tool_result(
            call.id.clone(),
            "lookup".try_into().unwrap(),
            vec![ToolResultContent::text("result")],
        )],
    };
    response.transcript = vec![
        Message::Assistant {
            id: Some("native-message".into()),
            content: vec![
                AssistantContent::Reasoning(reasoning.clone().sealed("openai")),
                AssistantContent::text("before tool"),
                tool_call.clone(),
            ],
        },
        tool_result.clone(),
        Message::assistant("after tool"),
    ];
    let outputs = response
        .output_blocks()
        .map(|(id, text)| (id.to_string(), format!("edited {text}")))
        .collect::<Vec<_>>();

    response.replace_editable_text(&[], &outputs).unwrap();

    assert_eq!(
        response.transcript,
        vec![
            Message::Assistant {
                id: Some("native-message".into()),
                content: vec![
                    AssistantContent::Reasoning(reasoning.sealed("openai")),
                    AssistantContent::text("edited before tool"),
                    tool_call
                ],
            },
            tool_result,
            Message::assistant("edited after tool"),
        ]
    );
    assert_eq!(
        response.output_text(),
        "edited before tooledited after tool"
    );
}

#[test]
fn unknown_provider_allows_output_edits_but_rejects_creating_reasoning_atomically() {
    let mut response = response();
    response.provider_kind = None;
    response.append_reasoning(None, "original reasoning", 0);
    response.append_output("original answer", 10);
    let reasoning_id = response.reasoning_blocks().next().unwrap().0.to_string();
    let output_id = response.output_blocks().next().unwrap().0.to_string();
    let before = response.clone();

    assert!(
        response
            .replace_editable_text(
                &[(reasoning_id, "edited reasoning".into())],
                &[(output_id.clone(), "edited answer".into())],
            )
            .unwrap_err()
            .contains("provider type")
    );
    assert_eq!(response, before);

    response
        .replace_editable_text(&[], &[(output_id, "edited answer".into())])
        .unwrap();
    assert_eq!(response.output_text(), "edited answer");
    assert_eq!(
        response.reasoning_blocks().next().unwrap().1,
        "original reasoning"
    );
}

#[test]
fn stopped_output_can_be_continued_without_rebuilding_blocks_or_repeating_the_prefill() {
    let mut response = response();
    let mut request = RequestInfo::new("conversation", "turn", &response.id);
    for event in [
        GenerationEvent::ThinkingDelta {
            provider_id: None,
            delta: "reasoning".into(),
        },
        GenerationEvent::TextDelta("partial".into()),
        GenerationEvent::Failed(GenerationError::cancelled()),
    ] {
        apply_event(
            event,
            &mut response,
            &mut request,
            Duration::from_millis(10),
        );
    }
    assert!(response.transcript.is_empty());
    assert_eq!(response.status, MessageStatus::Stopped);
    let blocks = response.blocks.clone();

    response.prepare_continuation();
    response.prepare_continuation();

    assert_eq!(response.blocks, blocks);
    assert_eq!(response.transcript, vec![Message::assistant("partial")]);
    apply_event(
        GenerationEvent::TextDelta(" continued".into()),
        &mut response,
        &mut request,
        Duration::from_millis(20),
    );
    apply_event(
        GenerationEvent::TranscriptContinued(Box::new(Message::assistant(" continued"))),
        &mut response,
        &mut request,
        Duration::from_millis(30),
    );
    apply_event(
        GenerationEvent::Completed,
        &mut response,
        &mut request,
        Duration::from_millis(40),
    );
    assert_eq!(response.output_text(), "partial continued");
    assert_eq!(
        response.transcript,
        vec![Message::assistant("partial continued")]
    );
    assert!(response.is_usable_as_context());
}

#[test]
fn output_usage_counts_unicode_characters_across_blocks_before_rounding() {
    let mut response = response();
    let mut request = RequestInfo::new("conversation", "turn", &response.id);
    response.append_reasoning(None, "想😀", 0);
    response.append_output("a", 10);
    response.observe_tool_call("call".into(), None, 20);
    response.append_output("b界", 30);

    apply_event(
        GenerationEvent::Completed,
        &mut response,
        &mut request,
        Duration::from_millis(40),
    );

    assert_eq!(request.usage.output_tokens, Some(2));
    assert!(request.usage.estimated);
    assert_eq!(response.output_text(), "ab界");
    assert_eq!(response.reasoning_blocks().next().unwrap().1, "想😀");
}
