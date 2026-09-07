use std::sync::Barrier;

use super::*;

fn conversation_path(storage: &Storage, id: &str) -> std::path::PathBuf {
    storage
        .conversations_dir()
        .join(id)
        .join(format!("{id}.json"))
}

#[test]
fn session_edits_use_memory_and_restart_reads_external_changes() {
    let (directory, storage) = open_storage();
    let conversation = Conversation::new("In memory", None, "original prompt");
    storage.insert_conversation(&conversation).unwrap();
    let path = conversation_path(&storage, &conversation.id);
    let mut external = storage.load_conversation(&conversation.id).unwrap();
    external.conversation.system_prompt = "external edit".into();
    fs::write(&path, serde_json::to_vec(&external).unwrap()).unwrap();

    let saved = storage
        .update_session(&conversation.id, |session| session.rename("New title"))
        .unwrap();
    assert_eq!(saved.conversation.title, "New title");
    assert_eq!(saved.conversation.system_prompt, "original prompt");
    assert_eq!(
        serde_json::from_slice::<onechat::domain::ConversationSession>(&fs::read(&path).unwrap())
            .unwrap(),
        saved
    );

    let source = serde_json::to_vec(&external).unwrap();
    fs::write(&path, &source).unwrap();
    assert_eq!(storage.load_conversation(&conversation.id).unwrap(), saved);
    let reopened = Storage::open(storage.settings_path(), directory.path().join("state")).unwrap();
    reopened.load_startup_snapshot().unwrap();
    assert_eq!(
        reopened.load_conversation(&conversation.id).unwrap(),
        external
    );
    assert_eq!(fs::read(&path).unwrap(), source);
}

#[test]
fn failed_session_write_does_not_commit_to_memory() {
    let (_directory, storage) = open_storage();
    let conversation = Conversation::new("Before", None, "");
    storage.insert_conversation(&conversation).unwrap();
    let before = storage.load_conversation(&conversation.id).unwrap();
    let path = conversation_path(&storage, &conversation.id);
    let source = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();

    assert!(
        storage
            .update_session(&conversation.id, |session| session
                .rename("Must not commit"))
            .is_err()
    );
    assert_eq!(storage.load_conversation(&conversation.id).unwrap(), before);
    fs::remove_dir(&path).unwrap();
    fs::write(&path, source).unwrap();
    let saved = storage
        .update_session(&conversation.id, |session| session.rename("Retry"))
        .unwrap();
    assert_eq!(saved.conversation.title, "Retry");
}

#[test]
fn concurrent_generation_and_metadata_edits_preserve_both_results() {
    let (_directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let conversation = Conversation::new("Before", Some(&model), "");
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
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            barrier.wait();
            for index in 0..16 {
                storage
                    .update_session(&conversation.id, |session| {
                        session.rename(&format!("Title {index}"))
                    })
                    .unwrap();
            }
        });
        scope.spawn(|| {
            barrier.wait();
            for index in 0..16 {
                prepared
                    .response
                    .append_output(&format!("Answer {index}"), index);
                prepared.response.status = MessageStatus::Streaming;
                prepared.request_info.status = RequestStatus::Streaming;
                storage
                    .persist_generation(&prepared.response, &prepared.request_info)
                    .unwrap();
            }
        });
    });
    let saved = storage.load_conversation(&conversation.id).unwrap();
    assert_eq!(saved.conversation.title, "Title 15");
    assert_eq!(saved.turns[0].responses[0], prepared.response);
    assert_eq!(saved.requests[0], prepared.request_info);
    assert_eq!(
        serde_json::from_slice::<onechat::domain::ConversationSession>(
            &fs::read(conversation_path(&storage, &conversation.id)).unwrap()
        )
        .unwrap(),
        saved
    );

    storage.delete_conversation(&conversation.id).unwrap();
    assert!(
        storage
            .persist_generation(&prepared.response, &prepared.request_info)
            .is_err()
    );
    assert!(storage.load_conversation(&conversation.id).is_err());
    assert!(!storage.conversations_dir().join(&conversation.id).exists());
}

#[test]
fn failed_fork_does_not_publish_a_session() {
    let (_directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let source = Conversation::new("Source", Some(&model), "");
    storage.insert_conversation(&source).unwrap();
    let prepared = prepare_turn(
        &storage,
        &source,
        &provider,
        &model,
        &[],
        None,
        UserMessage::new("question", Vec::new()),
    );
    let (_, response_id) = begin_and_complete(&storage, prepared, "answer");
    let attachments = storage
        .store_attachments(
            &source.id,
            &[AttachmentDraft {
                id: "note".into(),
                name: "note.txt".into(),
                kind: AttachmentKind::Text,
                files: vec![AttachmentDraftFile {
                    name: "note.txt".into(),
                    kind: AttachmentFileKind::Text,
                    media_type: "text/plain".into(),
                    bytes: b"notes".to_vec(),
                }],
                audio: None,
            }],
        )
        .unwrap();
    storage
        .update_session(&source.id, |session| {
            session.turns[0].user.attachments = attachments.clone();
            Ok(())
        })
        .unwrap();
    storage
        .remove_attachments(&source.id, &attachments)
        .unwrap();
    let fork = Conversation::new("Fork", Some(&model), "");
    assert!(
        storage
            .fork_conversation(&source.id, &response_id, &fork)
            .is_err()
    );
    assert!(storage.load_conversation(&fork.id).is_err());
    assert!(!storage.conversations_dir().join(&fork.id).exists());
}
