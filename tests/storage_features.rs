use std::fs;

use onechat::{
    application::generation::{ContextPolicy, GenerationStart, PreparedGeneration},
    domain::{
        AppSettings, AttachmentDraft, AttachmentDraftFile, AttachmentFileKind, AttachmentKind,
        AutoTitleState, Conversation, HistoryLimit, MessageStatus, Model, PromptVariableSource,
        Provider, ProviderKind, RequestContextInfo, RequestInfo, RequestStatus, SystemPromptPreset,
        ToolExecution, ToolExecutionStatus, Turn, UserMessage, active_turns,
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
fn context_policy_fields_are_backward_compatible_and_round_trip() {
    let settings: AppSettings = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(settings.history_limit, HistoryLimit::Unlimited);

    let model = Model::new("provider", "remote", "Model");
    let mut old_model = serde_json::to_value(&model).unwrap();
    old_model
        .as_object_mut()
        .unwrap()
        .remove("context_window_tokens");
    let old_model: Model = serde_json::from_value(old_model).unwrap();
    assert_eq!(old_model.context_window_tokens, None);
    assert_eq!(
        serde_json::from_value::<Model>(serde_json::to_value(&old_model).unwrap()).unwrap(),
        old_model
    );

    let conversation = Conversation::new("Chat", None, "");
    let mut old_conversation = serde_json::to_value(&conversation).unwrap();
    old_conversation
        .as_object_mut()
        .unwrap()
        .remove("history_limit_override");
    let old_conversation: Conversation = serde_json::from_value(old_conversation).unwrap();
    assert_eq!(old_conversation.history_limit_override, None);
    assert_eq!(
        old_conversation.effective_history_limit(HistoryLimit::Last(8)),
        HistoryLimit::Last(8)
    );

    let mut explicit_unlimited = old_conversation.clone();
    explicit_unlimited.history_limit_override = Some(HistoryLimit::Unlimited);
    let explicit_unlimited: Conversation =
        serde_json::from_value(serde_json::to_value(&explicit_unlimited).unwrap()).unwrap();
    assert_eq!(
        explicit_unlimited.history_limit_override,
        Some(HistoryLimit::Unlimited)
    );
    assert_eq!(
        explicit_unlimited.effective_history_limit(HistoryLimit::Last(8)),
        HistoryLimit::Unlimited
    );

    let request = RequestInfo::new("conversation", "turn", "response");
    let mut old_request = serde_json::to_value(&request).unwrap();
    let old_request_object = old_request.as_object_mut().unwrap();
    old_request_object.remove("context");
    old_request_object.remove("last_step_input_tokens");
    old_request_object.remove("last_step_estimated_input_tokens");
    let old_request: RequestInfo = serde_json::from_value(old_request).unwrap();
    assert_eq!(old_request.context, None);
    assert_eq!(old_request.last_step_input_tokens, None);
    assert_eq!(old_request.last_step_estimated_input_tokens, None);

    let context = RequestContextInfo {
        history_limit: HistoryLimit::Last(8),
        available_history_turns: 12,
        included_history_turns: 8,
        limited_by_context_window: false,
    };
    let mut request = old_request;
    request.context = Some(context);
    let request: RequestInfo =
        serde_json::from_value(serde_json::to_value(&request).unwrap()).unwrap();
    assert_eq!(request.context, Some(context));
}

#[test]
fn model_context_window_persists_and_can_be_cleared() {
    let (_directory, storage) = open_storage();
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    storage.insert_provider(&provider).unwrap();
    let mut model = Model::new(&provider.id, "model", "Model");
    model.context_window_tokens = Some(128_000);
    storage.insert_model(&model).unwrap();

    assert_eq!(
        storage.load_snapshot().unwrap().models[0].context_window_tokens,
        Some(128_000)
    );

    model.context_window_tokens = None;
    storage.update_model(&model).unwrap();
    assert_eq!(
        storage.load_snapshot().unwrap().models[0].context_window_tokens,
        None
    );
}

#[test]
fn storage_normalizes_out_of_range_history_limit() {
    let (_directory, storage) = open_storage();
    fs::write(
        storage.settings_path(),
        r#"{
            providers: [],
            models: [],
            history_limit: { mode: "last", turns: 999 },
        }"#,
    )
    .unwrap();

    let snapshot = storage.load_snapshot().unwrap();
    assert_eq!(snapshot.settings.history_limit, HistoryLimit::Last(50));
    assert_eq!(
        storage.load_snapshot().unwrap().settings.history_limit,
        HistoryLimit::Last(50)
    );
}

#[test]
fn global_history_limit_values_round_trip() {
    let (_directory, storage) = open_storage();
    let mut settings = storage.load_snapshot().unwrap().settings;

    for limit in [
        HistoryLimit::Last(0),
        HistoryLimit::Last(1),
        HistoryLimit::Last(50),
        HistoryLimit::Unlimited,
    ] {
        settings.history_limit = limit;
        storage.save_settings(&settings).unwrap();
        assert_eq!(
            storage.load_snapshot().unwrap().settings.history_limit,
            limit
        );
    }
}

#[test]
fn conversation_history_override_is_explicit_until_reset() {
    let (_directory, storage) = open_storage();
    let mut settings = storage.load_snapshot().unwrap().settings;
    settings.history_limit = HistoryLimit::Last(8);
    storage.save_settings(&settings).unwrap();
    let mut conversation = Conversation::new("Chat", None, "");
    storage.insert_conversation(&conversation).unwrap();

    assert_eq!(
        conversation.effective_history_limit(settings.history_limit),
        HistoryLimit::Last(8)
    );
    settings.history_limit = HistoryLimit::Last(3);
    storage.save_settings(&settings).unwrap();
    assert_eq!(
        conversation.effective_history_limit(settings.history_limit),
        HistoryLimit::Last(3)
    );

    conversation.history_limit_override = Some(HistoryLimit::Last(3));
    storage.update_conversation(&conversation).unwrap();
    settings.history_limit = HistoryLimit::Last(1);
    storage.save_settings(&settings).unwrap();
    let stored = storage
        .load_snapshot()
        .unwrap()
        .conversations
        .into_iter()
        .find(|stored| stored.id == conversation.id)
        .unwrap();
    assert_eq!(stored.history_limit_override, Some(HistoryLimit::Last(3)));
    assert_eq!(
        stored.effective_history_limit(settings.history_limit),
        HistoryLimit::Last(3)
    );

    conversation.history_limit_override = Some(HistoryLimit::Unlimited);
    storage.update_conversation(&conversation).unwrap();
    let stored = storage
        .load_snapshot()
        .unwrap()
        .conversations
        .into_iter()
        .find(|stored| stored.id == conversation.id)
        .unwrap();
    assert_eq!(
        stored.effective_history_limit(settings.history_limit),
        HistoryLimit::Unlimited
    );

    conversation.history_limit_override = None;
    storage.update_conversation(&conversation).unwrap();
    let stored = storage
        .load_snapshot()
        .unwrap()
        .conversations
        .into_iter()
        .find(|stored| stored.id == conversation.id)
        .unwrap();
    assert_eq!(stored.history_limit_override, None);
    assert_eq!(
        stored.effective_history_limit(settings.history_limit),
        HistoryLimit::Last(1)
    );
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

#[test]
fn provider_order_round_trips() {
    let (_directory, storage) = open_storage();
    let providers = ["Zulu", "Alpha", "Middle"].map(|name| {
        let provider = Provider::new(name, ProviderKind::OpenAi);
        storage.insert_provider(&provider).unwrap();
        provider
    });

    assert_eq!(
        storage
            .load_snapshot()
            .unwrap()
            .providers
            .iter()
            .map(|provider| provider.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Zulu", "Alpha", "Middle"]
    );

    let ordered_ids = [2, 0, 1].map(|index| providers[index].id.clone()).to_vec();
    storage.reorder_providers(&ordered_ids).unwrap();

    assert_eq!(
        storage
            .load_snapshot()
            .unwrap()
            .providers
            .iter()
            .map(|provider| provider.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Middle", "Zulu", "Alpha"]
    );
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
        ContextPolicy::new(HistoryLimit::Unlimited, &|user| {
            storage
                .message_for_user(&conversation.id, user, model.capabilities.vision)
                .map_err(|error| error.to_string())
        }),
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
    assert!(
        storage
            .load_snapshot()
            .unwrap()
            .settings
            .parse_document_images
    );

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
        parse_document_images: false,
        ..AppSettings::default()
    };
    settings.prompt_variables.insert(
        "owner".into(),
        PromptVariableSource::Text {
            value: "Orion".into(),
        },
    );
    settings.prompt_variables.insert(
        "repo".into(),
        PromptVariableSource::Command {
            script: "git status --short".into(),
            cwd: Some("/tmp/repo".into()),
            timeout_ms: 1_500,
        },
    );
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
    assert!(!snapshot.settings.parse_document_images);
    assert_eq!(snapshot.settings.theme_color, "#AF52DE");
    assert_eq!(
        snapshot.settings.prompt_variables,
        settings.prompt_variables
    );
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
    let mut conversation = Conversation::new("Source", Some(&model), "Be concise");
    conversation.history_limit_override = Some(HistoryLimit::Last(7));
    storage.insert_conversation(&conversation).unwrap();
    let mut settings = AppSettings {
        current_conversation_id: Some(conversation.id.clone()),
        ..AppSettings::default()
    };
    storage.save_settings(&settings).unwrap();

    let attachments = storage
        .store_attachments(
            &conversation.id,
            &[
                AttachmentDraft {
                    id: "notes".into(),
                    name: "notes.txt".into(),
                    kind: AttachmentKind::Text,
                    files: vec![AttachmentDraftFile {
                        name: "content.txt".into(),
                        kind: AttachmentFileKind::Text,
                        media_type: "text/plain".into(),
                        bytes: b"important context".to_vec(),
                    }],
                },
                AttachmentDraft {
                    id: "report".into(),
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
                            bytes: b"chart".to_vec(),
                        },
                    ],
                },
            ],
        )
        .unwrap();
    assert_eq!(attachments[0].files[0].name, "content.txt");
    assert_eq!(attachments[0].files[0].kind, AttachmentFileKind::Text);
    assert_eq!(
        attachments[0].files[0].path,
        "attachments/notes/content.txt"
    );
    let attachment_path = storage
        .attachment_path(&conversation.id, &attachments[0].files[0].path)
        .unwrap();
    let document_paths = attachments[1]
        .files
        .iter()
        .map(|file| {
            storage
                .attachment_path(&conversation.id, &file.path)
                .unwrap()
        })
        .collect::<Vec<_>>();

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

    let mut fork = conversation.clone();
    fork.id = "fork".into();
    fork.title = "Fork".into();
    storage
        .fork_conversation(&conversation.id, &old_response_id, &fork)
        .unwrap();
    settings.current_conversation_id = Some(fork.id.clone());
    storage.save_settings(&settings).unwrap();
    let snapshot = storage.load_snapshot().unwrap();
    assert_eq!(
        snapshot
            .conversations
            .iter()
            .find(|conversation| conversation.id == fork.id)
            .unwrap()
            .history_limit_override,
        Some(HistoryLimit::Last(7))
    );
    assert_eq!(snapshot.current_turns.len(), 2);
    assert_ne!(snapshot.current_turns[1].id, old_turn.id);
    assert_eq!(snapshot.current_turns[1].responses[0].content, "old answer");

    let message = storage
        .message_for_user(&fork.id, &snapshot.current_turns[0].user, false)
        .unwrap();
    let message = serde_json::to_string(&message).unwrap();
    assert!(message.contains("important context"));
    assert!(message.contains("![Chart](image-001.png)"));
    assert!(!message.contains("Embedded image from"));

    let message = storage
        .message_for_user(&fork.id, &snapshot.current_turns[0].user, true)
        .unwrap();
    assert!(
        serde_json::to_string(&message)
            .unwrap()
            .contains("Embedded image from report.docx: image-001.png")
    );
    for file in &snapshot.current_turns[0].user.attachments[1].files {
        assert!(
            storage
                .attachment_path(&fork.id, &file.path)
                .unwrap()
                .exists()
        );
    }

    let fork_attachments = snapshot.current_turns[0].user.attachments.clone();
    let fork_paths = fork_attachments
        .iter()
        .flat_map(|attachment| &attachment.files)
        .map(|file| storage.attachment_path(&fork.id, &file.path).unwrap())
        .collect::<Vec<_>>();
    storage
        .remove_attachments(&fork.id, &fork_attachments)
        .unwrap();
    assert!(fork_paths.iter().all(|path| !path.exists()));

    storage
        .clear_conversation_context(&conversation.id)
        .unwrap();
    assert!(!attachment_path.exists());
    assert!(document_paths.iter().all(|path| !path.exists()));
}

#[test]
fn attachment_storage_rejects_unsafe_or_duplicate_logical_names_atomically() {
    let (_directory, storage) = open_storage();
    let (_provider, model) = catalog(&storage);
    let conversation = Conversation::new("Attachments", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();

    let text_file = |name: &str| AttachmentDraftFile {
        name: name.into(),
        kind: AttachmentFileKind::Text,
        media_type: "text/plain".into(),
        bytes: b"content".to_vec(),
    };
    let image_file = |name: &str| AttachmentDraftFile {
        name: name.into(),
        kind: AttachmentFileKind::Image,
        media_type: "image/png".into(),
        bytes: b"image".to_vec(),
    };

    let error = storage
        .store_attachments(
            &conversation.id,
            &[
                AttachmentDraft {
                    id: "created-first".into(),
                    name: "notes.txt".into(),
                    kind: AttachmentKind::Text,
                    files: vec![text_file("content.txt")],
                },
                AttachmentDraft {
                    id: "duplicate".into(),
                    name: "pages.pdf".into(),
                    kind: AttachmentKind::Pdf,
                    files: vec![image_file("page.png"), image_file("page.png")],
                },
            ],
        )
        .unwrap_err();
    assert!(error.to_string().contains("duplicate attachment file name"));
    assert!(
        !storage
            .attachment_path(&conversation.id, "attachments/created-first/content.txt")
            .unwrap()
            .exists()
    );

    let error = storage
        .store_attachments(
            &conversation.id,
            &[AttachmentDraft {
                id: "unsafe".into(),
                name: "unsafe.txt".into(),
                kind: AttachmentKind::Text,
                files: vec![text_file("../content.txt")],
            }],
        )
        .unwrap_err();
    assert!(error.to_string().contains("invalid attachment file name"));

    let error = storage
        .store_attachments(
            &conversation.id,
            &[AttachmentDraft {
                id: "wrong-role".into(),
                name: "notes.txt".into(),
                kind: AttachmentKind::Text,
                files: vec![image_file("content.txt")],
            }],
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("text attachment must contain exactly one text file")
    );
}

#[test]
fn image_and_pdf_assets_keep_names_roles_and_message_content() {
    let (_directory, storage) = open_storage();
    let (_provider, model) = catalog(&storage);
    let conversation = Conversation::new("Visual attachments", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();

    let image_file = |name: &str| AttachmentDraftFile {
        name: name.into(),
        kind: AttachmentFileKind::Image,
        media_type: "image/png".into(),
        bytes: b"\x89PNG\r\n\x1a\n".to_vec(),
    };
    let attachments = storage
        .store_attachments(
            &conversation.id,
            &[
                AttachmentDraft {
                    id: "image".into(),
                    name: "photo.png".into(),
                    kind: AttachmentKind::Image,
                    files: vec![image_file("content.png")],
                },
                AttachmentDraft {
                    id: "pdf".into(),
                    name: "document.pdf".into(),
                    kind: AttachmentKind::Pdf,
                    files: vec![image_file("page-001.png"), image_file("page-002.png")],
                },
            ],
        )
        .unwrap();

    assert_eq!(attachments[0].files[0].name, "content.png");
    assert_eq!(attachments[0].files[0].kind, AttachmentFileKind::Image);
    assert_eq!(attachments[1].files[1].name, "page-002.png");
    assert_eq!(attachments[1].files[1].path, "attachments/pdf/page-002.png");

    let message = storage
        .message_for_user(&conversation.id, &UserMessage::new("", attachments), false)
        .unwrap();
    let json = serde_json::to_string(&message).unwrap();
    assert!(json.contains("Image attachment: photo.png"));
    assert!(json.contains("PDF attachment: document.pdf (2 pages)"));
    assert!(json.contains("Page 1"));
    assert!(json.contains("Page 2"));
}

#[test]
fn documents_send_markdown_and_conditionally_include_named_images() {
    let (_directory, storage) = open_storage();
    let (_provider, model) = catalog(&storage);
    let conversation = Conversation::new("Documents", Some(&model), "");
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
                        bytes: b"# Report\n![First](image-001.png)\n![Second](image-002.png)"
                            .to_vec(),
                    },
                    AttachmentDraftFile {
                        name: "image-002.png".into(),
                        kind: AttachmentFileKind::Image,
                        media_type: "image/png".into(),
                        bytes: b"second image".to_vec(),
                    },
                    AttachmentDraftFile {
                        name: "image-001.png".into(),
                        kind: AttachmentFileKind::Image,
                        media_type: "image/png".into(),
                        bytes: b"first image".to_vec(),
                    },
                ],
            }],
        )
        .unwrap();
    assert!(!attachments[0].kind.requires_vision());
    storage
        .store_attachments(
            &conversation.id,
            &[AttachmentDraft {
                id: "markdown-only".into(),
                name: "notes.docx".into(),
                kind: AttachmentKind::Document,
                files: vec![AttachmentDraftFile {
                    name: "content.md".into(),
                    kind: AttachmentFileKind::Text,
                    media_type: "text/markdown".into(),
                    bytes: b"No images".to_vec(),
                }],
            }],
        )
        .unwrap();

    let user = UserMessage::new("Summarize this", attachments.clone());
    let text = serde_json::to_string(
        &storage
            .message_for_user(&conversation.id, &user, false)
            .unwrap(),
    )
    .unwrap();
    assert!(text.contains("# Report"));
    assert!(text.contains("![First](image-001.png)"));
    assert!(!text.contains("Embedded image from"));
    assert!(!text.contains("Zmlyc3QgaW1hZ2U="));

    let visual = serde_json::to_string(
        &storage
            .message_for_user(&conversation.id, &user, true)
            .unwrap(),
    )
    .unwrap();
    let first = visual
        .find("Embedded image from report.docx: image-001.png")
        .unwrap();
    let second = visual
        .find("Embedded image from report.docx: image-002.png")
        .unwrap();
    assert!(first < second);
    assert!(visual.contains("Zmlyc3QgaW1hZ2U="));
    assert!(visual.contains("c2Vjb25kIGltYWdl"));
}

#[test]
fn documents_reject_invalid_shapes_and_only_require_present_included_resources() {
    let (_directory, storage) = open_storage();
    let (_provider, model) = catalog(&storage);
    let conversation = Conversation::new("Documents", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();

    let markdown = |name: &str, media_type: &str| AttachmentDraftFile {
        name: name.into(),
        kind: AttachmentFileKind::Text,
        media_type: media_type.into(),
        bytes: b"![Image](image-001.png)".to_vec(),
    };
    let image = |name: &str| AttachmentDraftFile {
        name: name.into(),
        kind: AttachmentFileKind::Image,
        media_type: "image/png".into(),
        bytes: b"image".to_vec(),
    };

    for (id, files, expected) in [
        (
            "missing-markdown",
            vec![image("image-001.png")],
            "must contain content.md",
        ),
        (
            "wrong-markdown-name",
            vec![markdown("document.md", "text/markdown")],
            "must be content.md",
        ),
        (
            "wrong-markdown-type",
            vec![markdown("content.md", "text/plain")],
            "text/markdown",
        ),
        (
            "multiple-markdown",
            vec![
                markdown("content.md", "text/markdown"),
                markdown("notes.md", "text/markdown"),
            ],
            "exactly one text file",
        ),
        (
            "duplicate-image",
            vec![
                markdown("content.md", "text/markdown"),
                image("image.png"),
                image("image.png"),
            ],
            "duplicate attachment file name",
        ),
    ] {
        let error = storage
            .store_attachments(
                &conversation.id,
                &[AttachmentDraft {
                    id: id.into(),
                    name: "invalid.docx".into(),
                    kind: AttachmentKind::Document,
                    files,
                }],
            )
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }

    let attachments = storage
        .store_attachments(
            &conversation.id,
            &[AttachmentDraft {
                id: "missing-image".into(),
                name: "missing.docx".into(),
                kind: AttachmentKind::Document,
                files: vec![
                    markdown("content.md", "text/markdown"),
                    image("image-001.png"),
                ],
            }],
        )
        .unwrap();
    let image_path = storage
        .attachment_path(&conversation.id, &attachments[0].files[1].path)
        .unwrap();
    fs::remove_file(image_path).unwrap();
    let user = UserMessage::new("", attachments);
    assert!(
        storage
            .message_for_user(&conversation.id, &user, false)
            .is_ok()
    );
    assert!(
        storage
            .message_for_user(&conversation.id, &user, true)
            .unwrap_err()
            .to_string()
            .contains("No such file")
    );

    let markdown_path = storage
        .attachment_path(&conversation.id, &user.attachments[0].files[0].path)
        .unwrap();
    fs::remove_file(markdown_path).unwrap();
    assert!(
        storage
            .message_for_user(&conversation.id, &user, false)
            .unwrap_err()
            .to_string()
            .contains("No such file")
    );
}

#[test]
fn automatic_title_can_restart_from_a_stored_conversation() {
    let (_directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let conversation = Conversation::new("Old title", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();

    let prepared = prepare_turn(
        &storage,
        &conversation,
        &provider,
        &model,
        &[],
        None,
        UserMessage::new("question", Vec::new()),
    );
    begin_and_complete(&storage, prepared, "answer");
    storage
        .rename_conversation(&conversation.id, "Manual title")
        .unwrap();

    assert_eq!(
        storage.restart_auto_title(&conversation.id).unwrap(),
        Some(("question".into(), "answer".into()))
    );
    assert_eq!(
        storage.load_snapshot().unwrap().conversations[0].auto_title_state,
        AutoTitleState::Running
    );
    assert_eq!(storage.restart_auto_title(&conversation.id).unwrap(), None);

    assert!(
        storage
            .finish_auto_title(&conversation.id, Some("Generated title"))
            .unwrap()
    );
    assert_eq!(
        storage.restart_auto_title(&conversation.id).unwrap(),
        Some(("question".into(), "answer".into()))
    );
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
        .message_for_user(&conversation.id, &UserMessage::new("", Vec::new()), false)
        .unwrap_err();
    assert!(error.to_string().contains("text or an attachment"));
}
