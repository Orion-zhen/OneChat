use super::*;
use onechat::domain::{Message, RequestKind};

#[test]
fn reopening_preserves_block_only_responses_and_search_uses_only_output_text() {
    let (directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let conversation = Conversation::new("Blocks", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();
    let mut prepared = prepare_turn(
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
    let response = &mut prepared.response;
    response.append_reasoning(None, "private reasoning", 0);
    response.append_output("first ", 10);
    response.observe_tool_call("call".into(), None, 20);
    response.append_output("second", 30);
    response.transcript = vec![Message::assistant("first second")];
    response.status = MessageStatus::Completed;
    prepared.request_info.status = RequestStatus::Completed;
    storage
        .persist_generation(response, &prepared.request_info)
        .unwrap();

    let file_path = storage
        .conversations_dir()
        .join(&conversation.id)
        .join(format!("{}.json", conversation.id));
    let file: serde_json::Value = serde_json::from_slice(&fs::read(file_path).unwrap()).unwrap();
    let saved_response = &file["turns"][0]["responses"][0];
    assert!(saved_response["blocks"].is_array());
    assert!(saved_response.get("content").is_none());
    assert!(saved_response.get("thinking").is_none());

    let reopened = Storage::open(storage.settings_path(), directory.path().join("state")).unwrap();
    let snapshot = reopened.load_startup_snapshot().unwrap();
    let saved = reopened.load_conversation(&conversation.id).unwrap();
    assert_eq!(saved.turns[0].responses[0], *response);
    let entry = snapshot
        .conversation_search
        .entries(&conversation.id)
        .iter()
        .find(|entry| entry.response_id.as_deref() == Some(&response.id))
        .unwrap();
    assert_eq!(entry.content, "first second");
    assert!(entry.matches_normalized("first second"));
    assert!(!entry.matches_normalized("private reasoning"));
}

#[test]
fn startup_recovers_partial_continuation_from_blocks_and_updates_native_output() {
    let (directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let conversation = Conversation::new("Recover continuation", Some(&model), "");
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
    begin_and_complete(&storage, prepared, "original");
    let session = storage.load_conversation(&conversation.id).unwrap();
    let loader = |user: &UserMessage| Ok(Message::user(user.content.clone()));
    let mut continuation = PreparedGeneration::continuation(
        &conversation,
        &provider,
        &model,
        &session.turns,
        &session.turns[0],
        &session.turns[0].responses[0],
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    assert_eq!(continuation.request_info.kind, RequestKind::Continue);
    storage
        .update_session(&conversation.id, |session| {
            session.begin_generation(
                &continuation.start,
                &continuation.response,
                &continuation.request_info,
            )
        })
        .unwrap();
    continuation
        .response
        .append_output(" partial continuation", 20);
    storage
        .persist_generation(&continuation.response, &continuation.request_info)
        .unwrap();
    let blocks = continuation.response.blocks.clone();

    let reopened = Storage::open(storage.settings_path(), directory.path().join("state")).unwrap();
    reopened.load_startup_snapshot().unwrap();
    let session = reopened.load_conversation(&conversation.id).unwrap();
    let response = &session.turns[0].responses[0];
    assert_eq!(response.blocks, blocks);
    assert_eq!(response.status, MessageStatus::Completed);
    assert_eq!(response.output_text(), "original partial continuation");
    assert_eq!(
        response.transcript,
        vec![Message::assistant("original partial continuation")]
    );
    assert!(session.turns[0].continuation_response().is_some());
    let request = session
        .requests
        .iter()
        .find(|request| request.id == continuation.request_info.id)
        .unwrap();
    assert_eq!(request.status, RequestStatus::Interrupted);

    let reopened = Storage::open(storage.settings_path(), directory.path().join("state")).unwrap();
    reopened.load_startup_snapshot().unwrap();
    assert_eq!(
        reopened.load_conversation(&conversation.id).unwrap(),
        session
    );
}
