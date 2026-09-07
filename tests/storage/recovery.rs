use super::*;

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
        .update_session(&conversation.id, |session| session.rename("Manual title"))
        .unwrap();

    let title_source = storage
        .restart_auto_title(&conversation.id)
        .unwrap()
        .unwrap();
    assert_eq!(title_source.len(), 1);
    assert_eq!(title_source[0].0.content, "question");
    assert_eq!(title_source[0].1, "answer");
    assert_eq!(
        storage
            .load_conversation(&conversation.id)
            .unwrap()
            .conversation
            .auto_title_state,
        AutoTitleState::Running
    );
    assert_eq!(storage.restart_auto_title(&conversation.id).unwrap(), None);

    assert!(
        storage
            .finish_auto_title(&conversation.id, Some("Generated title"))
            .unwrap()
            .is_some()
    );
    let title_source = storage
        .restart_auto_title(&conversation.id)
        .unwrap()
        .unwrap();
    assert_eq!(title_source.len(), 1);
    assert_eq!(title_source[0].0.content, "question");
    assert_eq!(title_source[0].1, "answer");
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
    storage
        .update_session(&conversation.id, |session| {
            session.begin_generation(&prepared.start, &prepared.response, &prepared.request_info)
        })
        .unwrap();

    let mut response = prepared.response;
    let mut execution =
        ToolExecution::new("provider-call", "server", "tool", serde_json::json!({}));
    execution.status = ToolExecutionStatus::Running;
    response.tool_executions.push(execution);
    storage
        .persist_generation(&response, &prepared.request_info)
        .unwrap();
    assert!(storage.claim_auto_title(&conversation.id).unwrap());

    let storage = Storage::open(storage.settings_path(), _directory.path().join("state")).unwrap();
    let snapshot = storage.load_startup_snapshot().unwrap();
    assert_eq!(
        snapshot.conversations[0].auto_title_state,
        AutoTitleState::Finished
    );
    assert_eq!(
        snapshot.current_turns()[0].responses[0].status,
        MessageStatus::Interrupted
    );
    assert_eq!(
        snapshot.current_turns()[0].responses[0].tool_executions[0].status,
        ToolExecutionStatus::Interrupted
    );
    assert!(
        snapshot.current_turns()[0].responses[0].tool_executions[0]
            .finished_at
            .is_some()
    );
    assert_eq!(
        snapshot.current_requests()[0].status,
        RequestStatus::Interrupted
    );
}

#[test]
fn reading_current_conversations_does_not_rewrite_the_file() {
    let (_directory, storage) = open_storage();
    let conversation = Conversation::new("Read only", None, "prompt");
    storage.insert_conversation(&conversation).unwrap();
    let path = storage
        .conversations_dir()
        .join(&conversation.id)
        .join(format!("{}.json", conversation.id));
    let source = format!(
        "// Keep formatting on reads\n{}",
        fs::read_to_string(&path).unwrap()
    );
    fs::write(&path, &source).unwrap();

    let storage = Storage::open(storage.settings_path(), _directory.path().join("state")).unwrap();
    assert_eq!(
        storage.load_startup_snapshot().unwrap().conversations,
        vec![conversation.clone()]
    );
    assert!(
        storage
            .load_conversation_turns(&conversation.id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(fs::read_to_string(path).unwrap(), source);
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
