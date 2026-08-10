use super::*;

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
