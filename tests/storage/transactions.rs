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
fn first_completed_response_becomes_context_until_explicitly_changed() {
    let (_directory, storage) = open_storage();
    let (provider, first_model) = catalog(&storage);
    let second_model = Model::new(&provider.id, "second-model", "Second Model", provider.kind);
    storage.insert_model(&second_model).unwrap();
    let conversation = Conversation::new("Chat", Some(&first_model), "");
    storage.insert_conversation(&conversation).unwrap();
    let first = prepare_turn(
        &storage,
        &conversation,
        &provider,
        &first_model,
        &[],
        None,
        UserMessage::new("question", Vec::new()),
    );
    let session = storage
        .update_session(&conversation.id, |session| {
            session.begin_generation(&first.start, &first.response, &first.request_info)
        })
        .unwrap();
    let loader = |user: &UserMessage| {
        storage
            .message_for_user(&conversation.id, user, false)
            .map_err(|error| error.to_string())
    };
    let second = PreparedGeneration::additional(
        &conversation,
        &provider,
        &second_model,
        &session.turns,
        &session.turns[0],
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    storage
        .update_session(&conversation.id, |session| {
            session.begin_generation(&second.start, &second.response, &second.request_info)
        })
        .unwrap();
    let mut second_response = second.response;
    second_response.status = MessageStatus::Completed;
    second_response.append_output("second answer", 0);
    let second_id = second_response.id.clone();
    let mut second_request = second.request_info;
    second_request.status = RequestStatus::Completed;
    storage
        .persist_generation(&second_response, &second_request)
        .unwrap();
    assert_eq!(
        storage.load_conversation(&conversation.id).unwrap().turns[0].continuation_response_id,
        Some(second_id.clone())
    );

    let mut first_response = first.response;
    first_response.status = MessageStatus::Completed;
    first_response.append_output("first answer", 0);
    let mut first_request = first.request_info;
    first_request.status = RequestStatus::Completed;
    storage
        .persist_generation(&first_response, &first_request)
        .unwrap();
    assert_eq!(
        storage.load_conversation(&conversation.id).unwrap().turns[0].continuation_response_id,
        Some(second_id)
    );

    let session = storage
        .update_session(&conversation.id, |session| {
            session.set_continuation_response(&first_request.turn_id, &first_response.id)
        })
        .unwrap();
    assert_eq!(
        session.turns[0].continuation_response_id,
        Some(first_response.id)
    );
}
