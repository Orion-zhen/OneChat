use std::{collections::BTreeMap, sync::Arc, time::Duration};

use onechat::{
    application::generation::{
        ContextPolicy, GenerationManager, GenerationStart, GenerationUpdate, PreparedGeneration,
        apply_event, history_for_new_turn, history_for_turn, history_preview_for_new_turn,
        run_generation,
    },
    domain::{
        AssistantResponse, Attachment, AttachmentDraft, AttachmentDraftFile, AttachmentFileKind,
        AttachmentKind, Conversation, CustomReasoningPreset, GenerationConfig, GenerationError,
        GenerationErrorKind, GenerationEvent, HistoryLimit, KnownReasoningFormat, Message,
        MessageStatus, Model, ModelReasoningConfig, PromptVariableSource, Provider, ProviderKind,
        ReasoningParameter, ReasoningParameterValue, RequestInfo, RequestStatus, TokenUsage, Turn,
        UserMessage, merge_json_patch,
    },
    mcp::McpManager,
    storage::Storage,
};
use serde_json::{Map, Value, json};
use tempfile::tempdir;
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

fn image_attachment() -> Attachment {
    Attachment {
        id: "image".into(),
        name: "image.png".into(),
        kind: AttachmentKind::Image,
        files: Vec::new(),
    }
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
        ContextPolicy::new(HistoryLimit::Unlimited, &|user| {
            Ok(Message::user(user.content.clone()))
        }),
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
        ContextPolicy::new(HistoryLimit::Unlimited, &|user| {
            Ok(Message::user(user.content.clone()))
        }),
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

    let history = history_for_new_turn(
        &[root, old_branch, selected_branch],
        HistoryLimit::Unlimited,
    );
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
fn history_limits_keep_recent_complete_turns_and_do_not_count_current_message() {
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
    let middle = completed_turn(
        &conversation,
        Some(root.responses[0].id.clone()),
        "middle question",
        "middle answer",
        &model,
        &provider,
    );
    let turns = [root, middle.clone()];
    let loader = |user: &UserMessage| {
        if user.content == "root question" {
            Err("excluded root was expanded".into())
        } else {
            Ok(Message::user(user.content.clone()))
        }
    };

    let prepared = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(middle.responses[0].id.clone()),
        UserMessage::new("current question", Vec::new()),
        ContextPolicy::new(HistoryLimit::Last(1), &loader),
    )
    .unwrap();
    let messages = serialized_messages(&prepared.provider_request.messages);
    assert_eq!(messages.len(), 3);
    assert!(messages[0].contains("middle question"));
    assert!(messages[1].contains("middle answer"));
    assert!(messages[2].contains("current question"));
    assert!(messages.iter().all(|message| !message.contains("root")));
    let context = prepared.request_info.context.unwrap();
    assert_eq!(context.history_limit, HistoryLimit::Last(1));
    assert_eq!(context.available_history_turns, 2);
    assert_eq!(context.included_history_turns, 1);

    let stateless = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(middle.responses[0].id.clone()),
        UserMessage::new("current question", Vec::new()),
        ContextPolicy::new(HistoryLimit::Last(0), &|user| {
            Ok(Message::user(user.content.clone()))
        }),
    )
    .unwrap();
    let messages = serialized_messages(&stateless.provider_request.messages);
    assert_eq!(messages.len(), 1);
    assert!(messages[0].contains("current question"));

    let oversized_limit = history_for_new_turn(&turns, HistoryLimit::Last(50));
    assert_eq!(oversized_limit.len(), 4);
    let preview = history_preview_for_new_turn(&turns, HistoryLimit::Last(1));
    assert_eq!(preview.available_turns, 2);
    assert_eq!(preview.included_turns, 1);
}

#[test]
fn history_turns_keep_complete_transcripts_as_the_truncation_unit() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let conversation = Conversation::new("Chat", Some(&model), "");
    let mut root = completed_turn(
        &conversation,
        None,
        "tool question",
        "fallback answer",
        &model,
        &provider,
    );
    root.responses[0].transcript = vec![
        Message::assistant("tool call marker"),
        Message::user("tool result marker"),
    ];

    let included = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        std::slice::from_ref(&root),
        Some(root.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Last(1), &|user| {
            Ok(Message::user(user.content.clone()))
        }),
    )
    .unwrap();
    let included = serialized_messages(&included.provider_request.messages).join("\n");
    assert!(included.contains("tool question"));
    assert!(included.contains("tool call marker"));
    assert!(included.contains("tool result marker"));
    assert!(!included.contains("fallback answer"));

    let excluded = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        std::slice::from_ref(&root),
        Some(root.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Last(0), &|user| {
            Ok(Message::user(user.content.clone()))
        }),
    )
    .unwrap();
    let excluded = serialized_messages(&excluded.provider_request.messages).join("\n");
    assert!(!excluded.contains("tool question"));
    assert!(!excluded.contains("tool call marker"));
    assert!(!excluded.contains("tool result marker"));
}

#[test]
fn model_context_window_trims_only_complete_oldest_turns_and_updates_request_info() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let conversation = Conversation::new("Chat", Some(&model), "system");
    let mut root = completed_turn(
        &conversation,
        None,
        "old tool question",
        "fallback answer",
        &model,
        &provider,
    );
    root.responses[0].transcript = vec![
        Message::assistant("old tool call"),
        Message::user("old tool result"),
    ];
    let recent = completed_turn(
        &conversation,
        Some(root.responses[0].id.clone()),
        "recent question",
        "recent answer",
        &model,
        &provider,
    );
    let turns = [root, recent.clone()];
    let loader = |user: &UserMessage| Ok(Message::user(user.content.clone()));
    let prepare = |limit| {
        PreparedGeneration::new(
            &conversation,
            &provider,
            &model,
            &turns,
            Some(recent.responses[0].id.clone()),
            UserMessage::new("current question", Vec::new()),
            ContextPolicy::new(limit, &loader),
        )
        .unwrap()
    };

    let full = prepare(HistoryLimit::Unlimited);
    let full_tokens = full.request_info.usage.input_tokens.unwrap();
    let one_turn_tokens = prepare(HistoryLimit::Last(1))
        .request_info
        .usage
        .input_tokens
        .unwrap();
    let current_tokens = prepare(HistoryLimit::Last(0))
        .request_info
        .usage
        .input_tokens
        .unwrap();

    let mut unknown_window = full.clone();
    unknown_window.finalize_context().unwrap();
    assert_eq!(unknown_window.provider_request.messages.len(), 6);

    let mut exact_window = full.clone();
    exact_window.provider_request.model.context_window_tokens = Some(full_tokens as u32);
    exact_window.finalize_context().unwrap();
    assert_eq!(exact_window.provider_request.messages.len(), 6);
    assert!(
        !exact_window
            .request_info
            .context
            .unwrap()
            .limited_by_context_window
    );

    let mut one_removed = full.clone();
    one_removed.provider_request.model.context_window_tokens = Some(one_turn_tokens as u32);
    one_removed.finalize_context().unwrap();
    let messages = serialized_messages(&one_removed.provider_request.messages).join("\n");
    assert!(!messages.contains("old tool question"));
    assert!(!messages.contains("old tool call"));
    assert!(!messages.contains("old tool result"));
    assert!(messages.contains("recent question"));
    let context = one_removed.request_info.context.unwrap();
    assert_eq!(context.available_history_turns, 2);
    assert_eq!(context.included_history_turns, 1);
    assert!(context.limited_by_context_window);
    assert_eq!(
        one_removed.request_info.usage.input_tokens,
        Some(one_turn_tokens)
    );

    let mut all_removed = full;
    all_removed.provider_request.model.context_window_tokens = Some(current_tokens as u32);
    all_removed.finalize_context().unwrap();
    let messages = serialized_messages(&all_removed.provider_request.messages).join("\n");
    assert_eq!(all_removed.provider_request.messages.len(), 1);
    assert!(messages.contains("current question"));
    assert!(!messages.contains("recent question"));
    assert_eq!(
        all_removed
            .request_info
            .context
            .unwrap()
            .included_history_turns,
        0
    );
    assert_eq!(
        all_removed.request_info.usage.input_tokens,
        Some(current_tokens)
    );
}

#[tokio::test]
async fn resolved_system_prompt_is_used_for_context_window_preflight() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let conversation = Conversation::new("Chat", Some(&model), "{{large}}");
    let mut prepared = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &[],
        None,
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Unlimited, &|user| {
            Ok(Message::user(user.content.clone()))
        }),
    )
    .unwrap();
    let unresolved_tokens = prepared.request_info.usage.input_tokens.unwrap();
    prepared.provider_request.model.context_window_tokens = Some(unresolved_tokens as u32);
    prepared.configure_prompt(
        BTreeMap::from([(
            "large".into(),
            PromptVariableSource::Text {
                value: "expanded ".repeat(500),
            },
        )]),
        Default::default(),
    );

    prepared
        .render_system_prompt(CancellationToken::new())
        .await
        .unwrap();
    assert!(prepared.request_info.usage.input_tokens.unwrap() > unresolved_tokens);
    let error = prepared.finalize_context().unwrap_err();
    assert_eq!(error.kind, GenerationErrorKind::ContextLengthExceeded);
    assert!(error.message.contains("current message"));
}

#[tokio::test]
async fn pre_provider_context_failure_is_persisted_without_removing_attachments() {
    let directory = tempdir().unwrap();
    let storage = Arc::new(
        Storage::open(
            directory.path().join("config/settings.jsonc"),
            directory.path().join("state"),
        )
        .unwrap(),
    );
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    storage.insert_provider(&provider).unwrap();
    let mut model = Model::new(&provider.id, "tiny-model", "Tiny Model");
    model.context_window_tokens = Some(1);
    storage.insert_model(&model).unwrap();
    let conversation = Conversation::new("Chat", Some(&model), "system");
    storage.insert_conversation(&conversation).unwrap();
    let mut settings = storage.load_snapshot().unwrap().settings;
    settings.current_conversation_id = Some(conversation.id.clone());
    storage.save_settings(&settings).unwrap();
    let attachments = storage
        .store_attachments(
            &conversation.id,
            &[AttachmentDraft {
                id: "document".into(),
                name: "notes.txt".into(),
                kind: AttachmentKind::Document,
                files: vec![AttachmentDraftFile {
                    name: "content.md".into(),
                    kind: AttachmentFileKind::Text,
                    media_type: "text/markdown".into(),
                    bytes: b"retained attachment".to_vec(),
                }],
            }],
        )
        .unwrap();
    let user = UserMessage::new("current message", attachments.clone());
    let prepared = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &[],
        None,
        user,
        ContextPolicy::new(HistoryLimit::Unlimited, &|user| {
            storage
                .message_for_user(&conversation.id, user, false)
                .map_err(|error| error.to_string())
        }),
    )
    .unwrap()
    .with_new_attachments(attachments.clone());
    let GenerationStart::NewTurn(turn) = &prepared.start else {
        panic!("expected a new turn");
    };
    storage.begin_turn(turn, &prepared.request_info).unwrap();

    let (sender, receiver) = async_channel::bounded(1);
    run_generation(
        prepared,
        storage.clone(),
        Arc::new(McpManager::new(directory.path().join("mcp.json"))),
        CancellationToken::new(),
        sender,
    )
    .await;
    let GenerationUpdate::Snapshot(snapshot) = receiver.recv().await.unwrap() else {
        panic!("expected a generation snapshot");
    };
    assert!(snapshot.terminal);
    assert_eq!(snapshot.request.status, RequestStatus::Failed);
    assert_eq!(
        snapshot
            .request
            .error
            .as_ref()
            .map(|error| error.kind.as_str()),
        Some("context_length_exceeded")
    );

    let snapshot = storage.load_snapshot().unwrap();
    let turn = snapshot
        .current_turns
        .iter()
        .find(|turn| turn.id == snapshot.current_requests[0].turn_id)
        .unwrap();
    assert_eq!(turn.user.attachments, attachments);
    storage
        .message_for_user(&conversation.id, &turn.user, false)
        .unwrap();
}

#[test]
fn additional_and_regenerated_responses_only_use_target_ancestors_and_user_message() {
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
    let target = completed_turn(
        &conversation,
        Some(root.responses[0].id.clone()),
        "target question",
        "old target answer",
        &model,
        &provider,
    );
    let descendant = completed_turn(
        &conversation,
        Some(target.responses[0].id.clone()),
        "descendant question",
        "descendant answer",
        &model,
        &provider,
    );
    let turns = [root, target.clone(), descendant];
    let loader = |user: &UserMessage| Ok(Message::user(user.content.clone()));

    let preview = history_for_turn(&turns, &target, HistoryLimit::Last(0));
    let preview = serialized_messages(&preview).join("\n");
    assert!(preview.contains("target question"));
    assert!(!preview.contains("root question"));

    let additional = PreparedGeneration::additional(
        &conversation,
        &provider,
        &model,
        &turns,
        &target,
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    let regenerated = PreparedGeneration::regenerate(
        &conversation,
        &provider,
        &model,
        &turns,
        &target,
        &target.responses[0],
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    for messages in [
        &additional.provider_request.messages,
        &regenerated.provider_request.messages,
    ] {
        let messages = serialized_messages(messages).join("\n");
        assert!(messages.contains("root question"));
        assert!(messages.contains("root answer"));
        assert!(messages.contains("target question"));
        assert!(!messages.contains("old target answer"));
        assert!(!messages.contains("descendant"));
    }
}

#[test]
fn visual_attachments_only_require_vision_when_their_turn_is_retained() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "text-model", "Text Model");
    let conversation = Conversation::new("Chat", Some(&model), "");
    let mut root = completed_turn(
        &conversation,
        None,
        "visual root",
        "root answer",
        &model,
        &provider,
    );
    root.user.attachments.push(image_attachment());
    let mut recent = completed_turn(
        &conversation,
        Some(root.responses[0].id.clone()),
        "recent text",
        "recent answer",
        &model,
        &provider,
    );
    let turns = [root, recent.clone()];
    let loader = |user: &UserMessage| Ok(Message::user(user.content.clone()));

    let mut excluded = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(recent.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Last(1), &loader),
    )
    .unwrap();
    let recent_only_tokens = excluded.request_info.usage.input_tokens.unwrap();
    excluded.finalize_context().unwrap();

    let mut window_trimmed = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(recent.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    window_trimmed.provider_request.model.context_window_tokens = Some(recent_only_tokens as u32);
    window_trimmed.finalize_context().unwrap();
    assert_eq!(
        window_trimmed
            .request_info
            .context
            .unwrap()
            .included_history_turns,
        1
    );

    let mut retained = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(recent.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    assert_eq!(
        retained.finalize_context().unwrap_err().kind,
        GenerationErrorKind::UnsupportedParameter
    );

    recent.user.attachments.push(image_attachment());
    let turns = [turns[0].clone(), recent.clone()];
    let mut retained = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(recent.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Last(1), &loader),
    )
    .unwrap();
    assert!(retained.finalize_context().is_err());

    let mut current = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(recent.responses[0].id.clone()),
        UserMessage::new("current", vec![image_attachment()]),
        ContextPolicy::new(HistoryLimit::Last(0), &loader),
    )
    .unwrap();
    assert!(current.finalize_context().is_err());
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
fn document_images_follow_each_generation_target_model() {
    let directory = tempdir().unwrap();
    let storage = Storage::open(
        directory.path().join("config/settings.jsonc"),
        directory.path().join("state"),
    )
    .unwrap();
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    storage.insert_provider(&provider).unwrap();
    let text_model = Model::new(&provider.id, "text-model", "Text Model");
    storage.insert_model(&text_model).unwrap();
    let mut vision_model = text_model.clone();
    vision_model.id = "vision-model".into();
    vision_model.remote_id = "vision-model".into();
    vision_model.display_name = "Vision Model".into();
    vision_model.capabilities.vision = true;
    storage.insert_model(&vision_model).unwrap();
    let conversation = Conversation::new("Documents", Some(&text_model), "");
    storage.insert_conversation(&conversation).unwrap();

    let attachments = storage
        .store_attachments(
            &conversation.id,
            &[AttachmentDraft {
                id: "document".into(),
                name: "report.docx".into(),
                kind: AttachmentKind::Document,
                files: vec![
                    AttachmentDraftFile {
                        name: "content.md".into(),
                        kind: AttachmentFileKind::Text,
                        media_type: "text/markdown".into(),
                        bytes: b"# Report\n![Chart](image-001.png)".to_vec(),
                    },
                    AttachmentDraftFile {
                        name: "image-001.png".into(),
                        kind: AttachmentFileKind::Image,
                        media_type: "image/png".into(),
                        bytes: b"image".to_vec(),
                    },
                ],
            }],
        )
        .unwrap();
    let user = UserMessage::new("Review", attachments);
    let text_message = |user: &UserMessage| {
        storage
            .message_for_user(&conversation.id, user, text_model.capabilities.vision)
            .map_err(|error| error.to_string())
    };
    let visual_message = |user: &UserMessage| {
        storage
            .message_for_user(&conversation.id, user, vision_model.capabilities.vision)
            .map_err(|error| error.to_string())
    };
    let assert_document_images = |prepared: &PreparedGeneration, included: bool| {
        let messages = serialized_messages(&prepared.provider_request.messages).join("\n");
        assert!(messages.contains("![Chart](image-001.png)"));
        assert_eq!(
            messages.contains("Embedded image from report.docx"),
            included
        );
        assert_eq!(messages.contains("aW1hZ2U="), included);
    };

    let text_new = PreparedGeneration::new(
        &conversation,
        &provider,
        &text_model,
        &[],
        None,
        user.clone(),
        ContextPolicy::new(HistoryLimit::Unlimited, &text_message),
    )
    .unwrap();
    let visual_new = PreparedGeneration::new(
        &conversation,
        &provider,
        &vision_model,
        &[],
        None,
        user,
        ContextPolicy::new(HistoryLimit::Unlimited, &visual_message),
    )
    .unwrap();
    assert_document_images(&text_new, false);
    assert_document_images(&visual_new, true);

    let GenerationStart::NewTurn(turn) = text_new.start else {
        panic!("expected a new turn");
    };
    let text_additional = PreparedGeneration::additional(
        &conversation,
        &provider,
        &text_model,
        std::slice::from_ref(&turn),
        &turn,
        ContextPolicy::new(HistoryLimit::Unlimited, &text_message),
    )
    .unwrap();
    let visual_additional = PreparedGeneration::additional(
        &conversation,
        &provider,
        &vision_model,
        std::slice::from_ref(&turn),
        &turn,
        ContextPolicy::new(HistoryLimit::Unlimited, &visual_message),
    )
    .unwrap();
    assert_document_images(&text_additional, false);
    assert_document_images(&visual_additional, true);

    let previous_response = turn.responses[0].clone();
    let text_regenerated = PreparedGeneration::regenerate(
        &conversation,
        &provider,
        &text_model,
        std::slice::from_ref(&turn),
        &turn,
        &previous_response,
        ContextPolicy::new(HistoryLimit::Unlimited, &text_message),
    )
    .unwrap();
    let visual_regenerated = PreparedGeneration::regenerate(
        &conversation,
        &provider,
        &vision_model,
        std::slice::from_ref(&turn),
        &turn,
        &previous_response,
        ContextPolicy::new(HistoryLimit::Unlimited, &visual_message),
    )
    .unwrap();
    assert_document_images(&text_regenerated, false);
    assert_document_images(&visual_regenerated, true);
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
