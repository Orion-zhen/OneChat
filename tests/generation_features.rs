use std::time::Duration;

use onechat::{
    application::generation::{
        GenerationManager, GenerationStart, PreparedGeneration, apply_event, history_for_new_turn,
    },
    domain::{
        AssistantResponse, Conversation, CustomReasoningPreset, GenerationConfig, GenerationError,
        GenerationEvent, KnownReasoningFormat, Message, MessageStatus, Model, ModelReasoningConfig,
        Provider, ProviderKind, ReasoningParameter, ReasoningParameterValue, RequestInfo,
        RequestStatus, Turn, UserMessage, merge_json_patch,
    },
};
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

fn completed_turn(
    conversation: &Conversation,
    parent_response_id: Option<String>,
    user: &str,
    answer: &str,
    model: &Model,
    provider: &Provider,
) -> Turn {
    let mut response = AssistantResponse::new(model, provider);
    response.content = answer.into();
    let mut turn = Turn::new(
        conversation,
        parent_response_id,
        UserMessage::new(user, Vec::new()),
        response,
    );
    turn.continuation_response_id = Some(turn.responses[0].id.clone());
    turn
}

fn serialized_messages(messages: &[Message]) -> Vec<String> {
    messages
        .iter()
        .map(|message| serde_json::to_string(message).unwrap())
        .collect()
}

#[test]
fn generation_preparation_uses_the_selected_history_and_model_capabilities() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let mut model = Model::new(&provider.id, "test-model", "Test Model");
    model.capabilities.top_k = false;
    let mut conversation = Conversation::new("Chat", Some(&model), "  Be concise.  ");
    conversation.generation_config = GenerationConfig {
        temperature: Some(0.4),
        top_k: Some(20),
        ..GenerationConfig::default()
    };

    let root = completed_turn(
        &conversation,
        None,
        "first question",
        "first answer",
        &model,
        &provider,
    );
    let root_response_id = root.responses[0].id.clone();
    let prepared = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &[root],
        Some(root_response_id.clone()),
        UserMessage::new("follow-up", Vec::new()),
        &|user| Ok(Message::user(user.content.clone())),
    )
    .unwrap();

    let GenerationStart::NewTurn(turn) = &prepared.start else {
        panic!("expected a new turn");
    };
    assert_eq!(
        turn.parent_response_id.as_deref(),
        Some(root_response_id.as_str())
    );
    assert_eq!(prepared.response.status, MessageStatus::Streaming);
    assert_eq!(
        prepared.response.request_id.as_deref(),
        Some(prepared.request_info.id.as_str())
    );
    assert_eq!(prepared.request_info.status, RequestStatus::Sending);
    assert!(prepared.request_info.usage.input_tokens.is_some());
    assert!(prepared.request_info.usage.estimated);
    assert_eq!(prepared.provider_request.system_prompt, "Be concise.");
    assert_eq!(prepared.provider_request.config.temperature, Some(0.4));
    assert_eq!(prepared.provider_request.config.top_k, None);

    let messages = serialized_messages(&prepared.provider_request.messages);
    assert_eq!(messages.len(), 3);
    assert!(messages[0].contains("first question"));
    assert!(messages[1].contains("first answer"));
    assert!(messages[2].contains("follow-up"));
}

#[test]
fn regeneration_uses_the_current_reasoning_preset() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let mut conversation = Conversation::new("Chat", Some(&model), "");
    conversation.generation_config.temperature = Some(0.2);
    conversation.generation_config.reasoning_preset = Some("original".into());

    let turn = completed_turn(&conversation, None, "question", "answer", &model, &provider);
    let previous_response = turn.responses[0].clone();
    conversation.generation_config.temperature = Some(0.9);
    conversation.generation_config.reasoning_preset = Some("current".into());

    let prepared = PreparedGeneration::regenerate(
        &conversation,
        &provider,
        &model,
        std::slice::from_ref(&turn),
        &turn,
        &previous_response,
        &|user| Ok(Message::user(user.content.clone())),
    )
    .unwrap();

    assert_eq!(prepared.provider_request.config.temperature, Some(0.2));
    assert_eq!(
        prepared.provider_request.config.reasoning_preset.as_deref(),
        Some("current")
    );
}

#[test]
fn new_turn_history_follows_the_selected_branch() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let conversation = Conversation::new("Chat", Some(&model), "");

    let root = completed_turn(
        &conversation,
        None,
        "root question",
        "root answer",
        &model,
        &provider,
    );
    let root_response_id = root.responses[0].id.clone();
    let mut old_branch = completed_turn(
        &conversation,
        Some(root_response_id.clone()),
        "old branch",
        "old answer",
        &model,
        &provider,
    );
    old_branch.selected = false;
    let selected_branch = completed_turn(
        &conversation,
        Some(root_response_id),
        "selected branch",
        "selected answer",
        &model,
        &provider,
    );

    let history = history_for_new_turn(&[root, old_branch, selected_branch]);
    let messages = serialized_messages(&history);
    assert_eq!(messages.len(), 4);
    assert!(messages[0].contains("root question"));
    assert!(messages[1].contains("root answer"));
    assert!(messages[2].contains("selected branch"));
    assert!(messages[3].contains("selected answer"));
    assert!(
        messages
            .iter()
            .all(|message| !message.contains("old branch"))
    );
}

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
fn reasoning_presets_compile_and_merge_into_request_parameters() {
    let known = ModelReasoningConfig::known(KnownReasoningFormat::AnthropicManualBudget);
    let (_, patch) = known.resolve_patch(Some("high")).unwrap();
    assert_eq!(
        Value::Object(patch),
        json!({"thinking": {"type": "enabled", "budget_tokens": 16384}})
    );

    let custom = ModelReasoningConfig::Custom {
        default_preset: "fast".into(),
        presets: vec![CustomReasoningPreset {
            id: "fast".into(),
            name: None,
            request_parameters: vec![ReasoningParameter {
                path: "reasoning.effort".into(),
                value: ReasoningParameterValue::String("low".into()),
            }],
            chat_template_kwargs: vec![ReasoningParameter {
                path: "thinking".into(),
                value: ReasoningParameterValue::Boolean(true),
            }],
        }],
    };
    custom.validate().unwrap();
    let (_, patch) = custom.resolve_patch(None).unwrap();
    assert_eq!(
        Value::Object(patch.clone()),
        json!({
            "reasoning": {"effort": "low"},
            "chat_template_kwargs": {"thinking": true}
        })
    );

    let mut request = Map::from_iter([
        ("temperature".into(), json!(0.8)),
        ("reasoning".into(), json!({"summary": "auto"})),
    ]);
    merge_json_patch(&mut request, patch);
    assert_eq!(request["temperature"], json!(0.8));
    assert_eq!(
        request["reasoning"],
        json!({"summary": "auto", "effort": "low"})
    );
}

#[test]
fn generation_manager_prevents_parallel_runs_and_stops_the_active_run() {
    let mut manager = GenerationManager::default();
    let cancellation = CancellationToken::new();
    assert!(manager.start(
        "conversation".into(),
        "request-1".into(),
        "response-1".into(),
        cancellation.clone(),
    ));
    assert!(!manager.start(
        "conversation".into(),
        "request-2".into(),
        "response-2".into(),
        CancellationToken::new(),
    ));
    assert!(manager.stop("conversation"));
    assert!(cancellation.is_cancelled());

    manager.finish("conversation", "another-request");
    assert!(manager.is_active("conversation"));
    manager.finish("conversation", "request-1");
    assert!(!manager.is_active("conversation"));
}
