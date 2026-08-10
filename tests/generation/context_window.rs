use super::*;

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
