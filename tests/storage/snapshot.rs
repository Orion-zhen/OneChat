use super::*;

#[test]
fn background_session_updates_do_not_replace_the_selected_session_or_settings() {
    let (_directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let selected = Conversation::new("Selected", Some(&model), "");
    let background = Conversation::new("Background", Some(&model), "");
    storage.insert_conversation(&selected).unwrap();
    storage.insert_conversation(&background).unwrap();
    storage
        .save_settings(&AppSettings {
            current_conversation_id: Some(selected.id.clone()),
            ..Default::default()
        })
        .unwrap();
    let mut snapshot = storage.load_startup_snapshot().unwrap();
    let current = snapshot.current.clone();
    let settings = snapshot.settings.clone();
    let prepared = prepare_turn(
        &storage,
        &background,
        &provider,
        &model,
        &[],
        None,
        UserMessage::new("background question", Vec::new()),
    );
    begin_and_complete(&storage, prepared, "background answer");
    let updated = storage.load_conversation(&background.id).unwrap();
    snapshot.apply_conversation(updated, Some(&selected.id));
    assert_eq!(snapshot.current, current);
    assert_eq!(snapshot.settings, settings);
    let entries = snapshot.conversation_search.entries(&background.id);
    assert!(
        entries
            .iter()
            .any(|entry| entry.content == "background question")
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.content == "background answer")
    );
    snapshot.remove_conversation(&background.id);
    assert!(
        snapshot
            .conversation_search
            .entries(&background.id)
            .is_empty()
    );
    assert_eq!(snapshot.current, current);
    snapshot.remove_conversation(&selected.id);
    assert!(snapshot.current.is_none());
    assert!(snapshot.current_turns().is_empty());
    assert!(snapshot.current_requests().is_empty());
}

#[test]
fn metadata_update_preserves_streaming_content_and_requests() {
    let (_directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let conversation = Conversation::new("Streaming", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();
    storage
        .save_settings(&AppSettings {
            current_conversation_id: Some(conversation.id.clone()),
            ..Default::default()
        })
        .unwrap();
    let mut prepared = prepare_turn(
        &storage,
        &conversation,
        &provider,
        &model,
        &[],
        None,
        UserMessage::new("question", Vec::new()),
    );
    let mut snapshot = storage.load_startup_snapshot().unwrap();
    let session = storage
        .update_session(&conversation.id, |session| {
            session.begin_generation(&prepared.start, &prepared.response, &prepared.request_info)
        })
        .unwrap();
    snapshot.apply_conversation(session, Some(&conversation.id));
    onechat::application::generation::apply_event(
        onechat::domain::GenerationEvent::TextDelta("not yet persisted".into()),
        &mut prepared.response,
        &mut prepared.request_info,
        std::time::Duration::from_millis(1),
    );
    snapshot
        .current
        .as_mut()
        .unwrap()
        .update_generation(&prepared.response, &prepared.request_info)
        .unwrap();
    let before = snapshot.current.clone().unwrap();
    let mut renamed = conversation;
    renamed.title = "New title".into();
    snapshot.update_conversation_summary(renamed.clone());
    let after = snapshot.current.unwrap();
    assert_eq!(after.turns, before.turns);
    assert_eq!(after.requests, before.requests);
    assert_eq!(after.conversation, renamed);
    assert_eq!(snapshot.conversations[0], renamed);
}

#[test]
fn selected_session_update_replaces_history_and_only_its_search_entries() {
    let (_directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let conversation = Conversation::new("Selected", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();
    let mut snapshot = storage.load_startup_snapshot().unwrap();
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
    snapshot.apply_conversation(
        storage.load_conversation(&conversation.id).unwrap(),
        Some(&conversation.id),
    );
    assert_eq!(snapshot.current_turns().len(), 1);
    assert_eq!(
        snapshot.conversation_search.entries(&conversation.id).len(),
        2
    );

    let cleared = storage
        .clear_conversation_context(&conversation.id)
        .unwrap();
    snapshot.apply_conversation(cleared, Some(&conversation.id));
    assert!(snapshot.current_turns().is_empty());
    assert!(snapshot.current_requests().is_empty());
    assert!(
        snapshot
            .conversation_search
            .entries(&conversation.id)
            .is_empty()
    );
}
