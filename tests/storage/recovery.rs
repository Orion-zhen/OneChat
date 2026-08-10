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
