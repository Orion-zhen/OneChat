use super::*;
use onechat::{
    application::generation::apply_event,
    domain::{ConversationSession, GenerationEvent, Message},
};
use std::time::Duration;

fn assert_same_session(memory: &ConversationSession, stored: &ConversationSession) {
    let mut expected = memory.clone();
    expected.conversation.temporary = false;
    assert_eq!(stored, &expected);
}

fn begin_both(storage: &Storage, memory: &mut ConversationSession, prepared: &PreparedGeneration) {
    memory
        .begin_generation(&prepared.start, &prepared.response, &prepared.request_info)
        .unwrap();
    let stored = storage
        .update_session(&memory.conversation.id, |session| {
            session.begin_generation(&prepared.start, &prepared.response, &prepared.request_info)
        })
        .unwrap();
    assert_same_session(memory, &stored);
}

fn complete_both(
    storage: &Storage,
    memory: &mut ConversationSession,
    prepared: &PreparedGeneration,
    text: &str,
) {
    let mut response = prepared.response.clone();
    let mut request = prepared.request_info.clone();
    apply_event(
        GenerationEvent::TextDelta(text.into()),
        &mut response,
        &mut request,
        Duration::from_millis(1),
    );
    apply_event(
        GenerationEvent::Completed,
        &mut response,
        &mut request,
        Duration::from_millis(2),
    );
    memory.update_generation(&response, &request).unwrap();
    storage.persist_generation(&response, &request).unwrap();
    assert_same_session(
        memory,
        &storage.load_conversation(&memory.conversation.id).unwrap(),
    );
}

#[test]
fn temporary_and_persistent_sessions_share_generation_and_branch_selection() {
    let (_directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let conversation = Conversation::new("Session", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();
    let mut temporary = conversation.clone();
    temporary.temporary = true;
    let mut memory = ConversationSession::new(temporary);
    let user_message = |user: &UserMessage| Ok(Message::user(user.content.clone()));
    let policy = ContextPolicy::new(HistoryLimit::Unlimited, &user_message);
    let root = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &[],
        None,
        UserMessage::new("question", Vec::new()),
        policy,
    )
    .unwrap();
    begin_both(&storage, &mut memory, &root);
    complete_both(&storage, &mut memory, &root, "first answer");

    let second_model = Model::new(&provider.id, "second", "Second", provider.kind);
    let additional = PreparedGeneration::additional(
        &conversation,
        &provider,
        &second_model,
        &memory.turns,
        &memory.turns[0],
        policy,
    )
    .unwrap();
    begin_both(&storage, &mut memory, &additional);
    complete_both(&storage, &mut memory, &additional, "second answer");

    memory
        .set_continuation_response(&root.request_info.turn_id, &additional.response.id)
        .unwrap();
    storage
        .update_session(&conversation.id, |session| {
            session.set_continuation_response(&root.request_info.turn_id, &additional.response.id)
        })
        .unwrap();
    assert_eq!(
        memory.turns[0].continuation_response_id.as_deref(),
        Some(additional.response.id.as_str())
    );
    assert_same_session(
        &memory,
        &storage.load_conversation(&conversation.id).unwrap(),
    );

    let mut branches = Vec::new();
    for question in ["branch one", "branch two"] {
        let branch = PreparedGeneration::new(
            &conversation,
            &provider,
            &model,
            &memory.turns,
            Some(additional.response.id.clone()),
            UserMessage::new(question, Vec::new()),
            policy,
        )
        .unwrap();
        begin_both(&storage, &mut memory, &branch);
        complete_both(&storage, &mut memory, &branch, question);
        branches.push(branch.request_info.turn_id);
    }
    memory.select_user_branch(&branches[0]).unwrap();
    storage
        .update_session(&conversation.id, |session| {
            session.select_user_branch(&branches[0])
        })
        .unwrap();
    assert_eq!(active_turns(&memory.turns)[1].id, branches[0]);
    assert_same_session(
        &memory,
        &storage.load_conversation(&conversation.id).unwrap(),
    );

    memory
        .set_continuation_response(&root.request_info.turn_id, &root.response.id)
        .unwrap();
    storage
        .update_session(&conversation.id, |session| {
            session.set_continuation_response(&root.request_info.turn_id, &root.response.id)
        })
        .unwrap();
    assert_eq!(active_turns(&memory.turns).len(), 1);
    memory.select_turn_path(&branches[1]).unwrap();
    storage
        .update_session(&conversation.id, |session| {
            session.select_turn_path(&branches[1])
        })
        .unwrap();
    assert_eq!(active_turns(&memory.turns)[1].id, branches[1]);
    assert_same_session(
        &memory,
        &storage.load_conversation(&conversation.id).unwrap(),
    );

    let regeneration = PreparedGeneration::regenerate(
        &conversation,
        &provider,
        &model,
        &memory.turns,
        &memory.turns[2],
        &memory.turns[2].responses[0],
        policy,
    )
    .unwrap();
    begin_both(&storage, &mut memory, &regeneration);
    complete_both(&storage, &mut memory, &regeneration, "regenerated");
    let mut edited = memory.turns[2].responses[0].clone();
    let block_id = match &edited.blocks[0] {
        onechat::domain::AssistantBlock::Output { id, .. } => id.clone(),
        _ => panic!("expected output"),
    };
    edited
        .replace_editable_text(&[], &[(block_id, "edited".into())])
        .unwrap();
    memory.update_response(&branches[1], &edited).unwrap();
    storage
        .update_session(&conversation.id, |session| {
            session.update_response(&branches[1], &edited)
        })
        .unwrap();
    assert_same_session(
        &memory,
        &storage.load_conversation(&conversation.id).unwrap(),
    );

    memory.clear();
    let stored = storage
        .clear_conversation_context(&conversation.id)
        .unwrap();
    assert_same_session(&memory, &stored);
}

#[test]
fn targeted_update_does_not_read_other_conversations() {
    let (_directory, storage) = open_storage();
    let conversation = Conversation::new("Target", None, "");
    storage.insert_conversation(&conversation).unwrap();
    let unrelated = Conversation::new("Unrelated", None, "");
    storage.insert_conversation(&unrelated).unwrap();
    let path = storage
        .conversations_dir()
        .join(&unrelated.id)
        .join(format!("{}.json", unrelated.id));
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["id"] = serde_json::json!("wrong-id");
    fs::write(path, serde_json::to_string(&value).unwrap()).unwrap();
    let updated = storage
        .update_session(&conversation.id, |session| session.rename("Updated"))
        .unwrap();
    assert_eq!(updated.conversation.title, "Updated");
    assert_eq!(
        storage.load_conversation(&conversation.id).unwrap(),
        updated
    );
}
