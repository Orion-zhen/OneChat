use super::*;

#[test]
fn failed_settings_edit_does_not_write_partial_changes() {
    let (_directory, storage) = open_storage();
    let providers = ["First", "Second", "Third"].map(|name| {
        let provider = Provider::new(name, ProviderKind::OpenAi);
        storage.insert_provider(&provider).unwrap();
        provider
    });
    let before = fs::read(storage.settings_path()).unwrap();
    let invalid_order = vec![
        providers[0].id.clone(),
        "missing-provider".into(),
        providers[2].id.clone(),
    ];

    assert!(storage.reorder_providers(&invalid_order).is_err());
    assert_eq!(fs::read(storage.settings_path()).unwrap(), before);
}

#[test]
fn continuation_validation_promotion_and_failed_conversation_edit_are_atomic() {
    let (_directory, storage) = open_storage();
    let (provider, first_model) = catalog(&storage);
    let second_model = Model::new(&provider.id, "second-model", "Second Model");
    storage.insert_model(&second_model).unwrap();
    let conversation = Conversation::new("Chat", Some(&first_model), "");
    storage.insert_conversation(&conversation).unwrap();
    storage
        .save_settings(&AppSettings {
            current_conversation_id: Some(conversation.id.clone()),
            ..AppSettings::default()
        })
        .unwrap();

    let first = prepare_turn(
        &storage,
        &conversation,
        &provider,
        &first_model,
        &[],
        None,
        UserMessage::new("question", Vec::new()),
    );
    let GenerationStart::NewTurn(first_turn) = first.start else {
        panic!("expected a new turn");
    };
    storage
        .begin_turn(&first_turn, &first.request_info)
        .unwrap();
    let first_id = first.response.id.clone();

    let mut first_response = first.response;
    first_response.status = MessageStatus::Completed;
    storage
        .update_response(&conversation.id, &first_turn.id, &first_response)
        .unwrap();
    assert!(
        storage
            .set_continuation_response(&conversation.id, &first_turn.id, &first_id)
            .is_err()
    );

    first_response.status = MessageStatus::Streaming;
    first_response.content = "partial".into();
    storage
        .update_response(&conversation.id, &first_turn.id, &first_response)
        .unwrap();
    assert!(
        storage
            .set_continuation_response(&conversation.id, &first_turn.id, &first_id)
            .is_err()
    );

    let snapshot = storage.load_snapshot().unwrap();
    let turn = &snapshot.current_turns[0];
    let loader = |user: &UserMessage| {
        storage
            .message_for_user(&conversation.id, user, false)
            .map_err(|error| error.to_string())
    };
    let second = PreparedGeneration::additional(
        &conversation,
        &provider,
        &second_model,
        &snapshot.current_turns,
        turn,
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    let GenerationStart::AddResponse { turn_id } = &second.start else {
        panic!("expected an additional response");
    };
    storage
        .begin_response(
            &conversation.id,
            turn_id,
            &second.response,
            &second.request_info,
        )
        .unwrap();
    let mut second_response = second.response;
    second_response.status = MessageStatus::Completed;
    second_response.content = "second answer".into();
    let second_id = second_response.id.clone();
    let mut second_request = second.request_info;
    second_request.status = RequestStatus::Completed;
    storage
        .persist_generation(&second_response, &second_request)
        .unwrap();
    assert_eq!(
        storage.load_snapshot().unwrap().current_turns[0].continuation_response_id,
        Some(second_id.clone())
    );

    first_response.status = MessageStatus::Completed;
    first_response.content = "first answer".into();
    let mut first_request = first.request_info;
    first_request.status = RequestStatus::Completed;
    storage
        .persist_generation(&first_response, &first_request)
        .unwrap();
    assert_eq!(
        storage.load_snapshot().unwrap().current_turns[0].continuation_response_id,
        Some(second_id)
    );

    storage
        .set_continuation_response(&conversation.id, &first_turn.id, &first_id)
        .unwrap();
    let path = storage
        .conversations_dir()
        .join(&conversation.id)
        .join(format!("{}.json", conversation.id));
    let before = fs::read(&path).unwrap();
    first_response.content = "must not be written".into();
    assert!(
        storage
            .begin_regeneration(
                &conversation.id,
                &first_turn.id,
                &first_response,
                &first_request,
            )
            .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), before);
}
