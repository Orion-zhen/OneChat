use std::fs;

use onechat::{
    application::generation::{GenerationStart, PreparedGeneration},
    domain::{
        AppSettings, AttachmentDraft, AttachmentDraftFile, AttachmentKind, AutoTitleState,
        Conversation, MessageStatus, Model, Provider, ProviderKind, RequestStatus,
        SystemPromptPreset, ToolExecution, ToolExecutionStatus, Turn, UserMessage, active_turns,
    },
    storage::{Storage, WindowMode, WindowState},
};
use tempfile::{TempDir, tempdir};

fn open_storage() -> (TempDir, Storage) {
    let directory = tempdir().unwrap();
    let storage = Storage::open(
        directory.path().join("config/settings.jsonc"),
        directory.path().join("state"),
    )
    .unwrap();
    (directory, storage)
}

#[test]
fn window_state_round_trips() {
    let (_directory, storage) = open_storage();
    assert_eq!(storage.load_window_state().unwrap(), None);

    let state = WindowState {
        mode: WindowMode::Maximized,
        display: Some("display-id".into()),
        x: 120.0,
        y: 80.0,
        width: 1380.0,
        height: 900.0,
    };
    storage.save_window_state(&state).unwrap();

    assert_eq!(storage.load_window_state().unwrap(), Some(state));
}

fn catalog(storage: &Storage) -> (Provider, Model) {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    storage.insert_provider(&provider).unwrap();
    let model = Model::new(&provider.id, "test-model", "Test Model");
    storage.insert_model(&model).unwrap();
    (provider, model)
}

fn prepare_turn(
    storage: &Storage,
    conversation: &Conversation,
    provider: &Provider,
    model: &Model,
    turns: &[Turn],
    parent_response_id: Option<String>,
    user: UserMessage,
) -> PreparedGeneration {
    PreparedGeneration::new(
        conversation,
        provider,
        model,
        turns,
        parent_response_id,
        user,
        &|user| {
            storage
                .message_for_user(&conversation.id, user)
                .map_err(|error| error.to_string())
        },
    )
    .unwrap()
}

fn begin_and_complete(
    storage: &Storage,
    prepared: PreparedGeneration,
    answer: &str,
) -> (Turn, String) {
    let GenerationStart::NewTurn(turn) = prepared.start else {
        panic!("expected a new turn");
    };
    storage.begin_turn(&turn, &prepared.request_info).unwrap();

    let mut response = prepared.response;
    response.content = answer.into();
    response.status = MessageStatus::Completed;
    let mut request = prepared.request_info;
    request.status = RequestStatus::Completed;
    storage.persist_generation(&response, &request).unwrap();
    (*turn, response.id)
}

#[test]
fn catalog_settings_and_prompt_presets_round_trip() {
    let (_directory, storage) = open_storage();
    fs::write(
        storage.settings_path(),
        "{\n  // JSONC is accepted\n  providers: [],\n  models: [],\n}\n",
    )
    .unwrap();

    let (provider, model) = catalog(&storage);
    let conversation = Conversation::new("First chat", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();
    let mut settings = AppSettings {
        current_conversation_id: Some(conversation.id.clone()),
        primary_model_id: Some(model.id.clone()),
        title_generation_model_id: Some(model.id.clone()),
        title_generation_reasoning_preset: Some("low".into()),
        theme_color: "#AF52DE".into(),
        code_block_wrap: true,
        ..AppSettings::default()
    };
    storage.save_settings(&settings).unwrap();

    storage
        .insert_prompt_preset(&SystemPromptPreset::new("Concise", " Be brief. "))
        .unwrap();
    storage
        .update_prompt_preset(
            "Concise",
            &SystemPromptPreset::new("Direct", "Answer directly."),
        )
        .unwrap();

    let snapshot = storage.load_snapshot().unwrap();
    assert_eq!(snapshot.providers, vec![provider.clone()]);
    assert_eq!(snapshot.models, vec![model.clone()]);
    assert_eq!(snapshot.conversations, vec![conversation]);
    assert_eq!(
        snapshot.prompt_presets,
        vec![SystemPromptPreset::new("Direct", "Answer directly.")]
    );
    assert_eq!(snapshot.settings.primary_model_id, Some(model.id.clone()));
    assert!(snapshot.settings.code_block_wrap);
    assert_eq!(snapshot.settings.theme_color, "#AF52DE");
    assert_eq!(
        snapshot
            .settings
            .title_generation_reasoning_preset
            .as_deref(),
        Some("low")
    );

    storage.delete_provider(&provider.id).unwrap();
    let snapshot = storage.load_snapshot().unwrap();
    assert!(snapshot.providers.is_empty());
    assert!(snapshot.models.is_empty());
    assert_eq!(snapshot.conversations[0].model_id, None);
    assert_eq!(snapshot.settings.primary_model_id, None);
    assert_eq!(snapshot.settings.title_generation_model_id, None);

    storage.delete_prompt_preset("Direct").unwrap();
    settings.current_conversation_id = None;
    storage.save_settings(&settings).unwrap();
    assert!(storage.load_snapshot().unwrap().prompt_presets.is_empty());
}

#[test]
fn conversations_branch_fork_and_keep_attachment_content() {
    let (_directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let conversation = Conversation::new("Source", Some(&model), "Be concise");
    storage.insert_conversation(&conversation).unwrap();
    let mut settings = AppSettings {
        current_conversation_id: Some(conversation.id.clone()),
        ..AppSettings::default()
    };
    storage.save_settings(&settings).unwrap();

    let attachments = storage
        .store_attachments(
            &conversation.id,
            &[AttachmentDraft {
                id: "notes".into(),
                name: "notes.txt".into(),
                kind: AttachmentKind::Text,
                files: vec![AttachmentDraftFile {
                    extension: "txt",
                    media_type: "text/plain",
                    bytes: b"important context".to_vec(),
                }],
            }],
        )
        .unwrap();
    let attachment_path = storage
        .attachment_path(&conversation.id, &attachments[0].files[0].path)
        .unwrap();

    let prepared = prepare_turn(
        &storage,
        &conversation,
        &provider,
        &model,
        &[],
        None,
        UserMessage::new("root question", attachments),
    );
    let (_, root_response_id) = begin_and_complete(&storage, prepared, "root answer");

    let turns = storage.load_snapshot().unwrap().current_turns;
    let old = prepare_turn(
        &storage,
        &conversation,
        &provider,
        &model,
        &turns,
        Some(root_response_id.clone()),
        UserMessage::new("old branch", Vec::new()),
    );
    let (old_turn, old_response_id) = begin_and_complete(&storage, old, "old answer");

    let turns = storage.load_snapshot().unwrap().current_turns;
    let selected = prepare_turn(
        &storage,
        &conversation,
        &provider,
        &model,
        &turns,
        Some(root_response_id),
        UserMessage::new("selected branch", Vec::new()),
    );
    let (selected_turn, _) = begin_and_complete(&storage, selected, "selected answer");

    let snapshot = storage.load_snapshot().unwrap();
    let active = active_turns(&snapshot.current_turns);
    assert_eq!(active.len(), 2);
    assert_eq!(active[1].id, selected_turn.id);
    storage
        .select_user_branch(&conversation.id, &old_turn.id)
        .unwrap();
    let snapshot = storage.load_snapshot().unwrap();
    assert_eq!(active_turns(&snapshot.current_turns)[1].id, old_turn.id);

    let fork = Conversation::new("Fork", Some(&model), "Be concise");
    storage
        .fork_conversation(&conversation.id, &old_response_id, &fork)
        .unwrap();
    settings.current_conversation_id = Some(fork.id.clone());
    storage.save_settings(&settings).unwrap();
    let snapshot = storage.load_snapshot().unwrap();
    assert_eq!(snapshot.current_turns.len(), 2);
    assert_ne!(snapshot.current_turns[1].id, old_turn.id);
    assert_eq!(snapshot.current_turns[1].responses[0].content, "old answer");

    let message = storage
        .message_for_user(&fork.id, &snapshot.current_turns[0].user)
        .unwrap();
    assert!(
        serde_json::to_string(&message)
            .unwrap()
            .contains("important context")
    );

    storage
        .clear_conversation_context(&conversation.id)
        .unwrap();
    assert!(!attachment_path.exists());
}

#[test]
fn startup_recovers_interrupted_generation_and_auto_title() {
    let (_directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let conversation = Conversation::new("Recover me", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();
    storage
        .save_settings(&AppSettings {
            current_conversation_id: Some(conversation.id.clone()),
            ..AppSettings::default()
        })
        .unwrap();

    let prepared = prepare_turn(
        &storage,
        &conversation,
        &provider,
        &model,
        &[],
        None,
        UserMessage::new("question", Vec::new()),
    );
    let GenerationStart::NewTurn(turn) = prepared.start else {
        panic!("expected a new turn");
    };
    storage.begin_turn(&turn, &prepared.request_info).unwrap();

    let mut response = prepared.response;
    let mut execution = ToolExecution::new(
        "provider-call",
        None,
        "server",
        "tool",
        serde_json::json!({}),
    );
    execution.status = ToolExecutionStatus::Running;
    response.tool_executions.push(execution);
    storage
        .persist_generation(&response, &prepared.request_info)
        .unwrap();
    assert!(storage.claim_auto_title(&conversation.id).unwrap());

    let snapshot = storage.load_startup_snapshot().unwrap();
    assert_eq!(
        snapshot.conversations[0].auto_title_state,
        AutoTitleState::Finished
    );
    assert_eq!(
        snapshot.current_turns[0].responses[0].status,
        MessageStatus::Interrupted
    );
    assert_eq!(
        snapshot.current_turns[0].responses[0].tool_executions[0].status,
        ToolExecutionStatus::Interrupted
    );
    assert!(
        snapshot.current_turns[0].responses[0].tool_executions[0]
            .finished_at
            .is_some()
    );
    assert_eq!(
        snapshot.current_requests[0].status,
        RequestStatus::Interrupted
    );
}

#[test]
fn empty_user_messages_are_rejected() {
    let (_directory, storage) = open_storage();
    let (_, model) = catalog(&storage);
    let conversation = Conversation::new("Chat", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();

    let error = storage
        .message_for_user(&conversation.id, &UserMessage::new("", Vec::new()))
        .unwrap_err();
    assert!(error.to_string().contains("text or an attachment"));
}
