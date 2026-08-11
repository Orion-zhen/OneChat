use super::*;

#[test]
fn attachment_storage_rejects_unsafe_or_duplicate_logical_names_atomically() {
    let (_directory, storage) = open_storage();
    let (_provider, model) = catalog(&storage);
    let conversation = Conversation::new("Attachments", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();

    let text_file = |name: &str| AttachmentDraftFile {
        name: name.into(),
        kind: AttachmentFileKind::Text,
        media_type: "text/plain".into(),
        bytes: b"content".to_vec(),
    };
    let image_file = |name: &str| AttachmentDraftFile {
        name: name.into(),
        kind: AttachmentFileKind::Image,
        media_type: "image/png".into(),
        bytes: b"image".to_vec(),
    };

    let error = storage
        .store_attachments(
            &conversation.id,
            &[
                AttachmentDraft {
                    id: "created-first".into(),
                    name: "notes.txt".into(),
                    kind: AttachmentKind::Text,
                    files: vec![text_file("content.txt")],
                    audio: None,
                },
                AttachmentDraft {
                    id: "duplicate".into(),
                    name: "pages.pdf".into(),
                    kind: AttachmentKind::Pdf,
                    files: vec![image_file("page.png"), image_file("page.png")],
                    audio: None,
                },
            ],
        )
        .unwrap_err();
    assert!(error.to_string().contains("duplicate attachment file name"));
    assert!(
        !storage
            .attachment_path(&conversation.id, "attachments/created-first/content.txt")
            .unwrap()
            .exists()
    );

    let error = storage
        .store_attachments(
            &conversation.id,
            &[AttachmentDraft {
                id: "unsafe".into(),
                name: "unsafe.txt".into(),
                kind: AttachmentKind::Text,
                files: vec![text_file("../content.txt")],
                audio: None,
            }],
        )
        .unwrap_err();
    assert!(error.to_string().contains("invalid attachment file name"));

    let error = storage
        .store_attachments(
            &conversation.id,
            &[AttachmentDraft {
                id: "wrong-role".into(),
                name: "notes.txt".into(),
                kind: AttachmentKind::Text,
                files: vec![image_file("content.txt")],
                audio: None,
            }],
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("text attachment must contain exactly one text file")
    );
}

#[test]
fn audio_attachment_lifecycle_persists_replays_forks_clears_and_deletes() {
    let (_directory, storage) = open_storage();
    let (provider, mut model) = catalog(&storage);
    model.capabilities.audio_input = true;
    let conversation = Conversation::new("Audio", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();

    let bytes = b"audio bytes".to_vec();
    let attachments = storage
        .store_attachments(
            &conversation.id,
            &[AttachmentDraft {
                id: "audio".into(),
                name: "speech.wav".into(),
                kind: AttachmentKind::Audio,
                files: vec![AttachmentDraftFile {
                    name: "content.wav".into(),
                    kind: AttachmentFileKind::Audio,
                    media_type: "audio/wav".into(),
                    bytes: bytes.clone(),
                }],
                audio: Some(AudioAttachmentMetadata {
                    duration_ms: 1_250,
                    source: AudioAttachmentSource::Upload,
                }),
            }],
        )
        .unwrap();
    assert_eq!(
        attachments[0].audio,
        Some(AudioAttachmentMetadata {
            duration_ms: 1_250,
            source: AudioAttachmentSource::Upload,
        })
    );

    let source = storage
        .attachment_path(&conversation.id, &attachments[0].files[0].path)
        .unwrap();
    let mut response = AssistantResponse::new(&model, &provider);
    response.content = "transcript".into();
    let mut turn = Turn::new(
        &conversation,
        None,
        UserMessage::new("Transcribe", attachments.clone()),
        response,
    );
    let request = RequestInfo::new(&conversation.id, &turn.id, &turn.responses[0].id);
    turn.responses[0].request_id = Some(request.id.clone());
    storage.begin_turn(&turn, &request).unwrap();

    let mut continued = prepare_turn(
        &storage,
        &conversation,
        &provider,
        &model,
        &[turn.clone()],
        Some(turn.responses[0].id.clone()),
        UserMessage::new("Continue", Vec::new()),
    );
    continued.finalize_context().unwrap();
    assert_eq!(continued.provider_request.audio_duration_ms, 1_250);
    assert!(
        serde_json::to_string(&continued.provider_request.messages)
            .unwrap()
            .contains("YXVkaW8gYnl0ZXM=")
    );

    let mut text_model = model.clone();
    text_model.capabilities.audio_input = false;
    let mut incompatible = prepare_turn(
        &storage,
        &conversation,
        &provider,
        &text_model,
        &[turn.clone()],
        Some(turn.responses[0].id.clone()),
        UserMessage::new("Continue", Vec::new()),
    );
    let error = incompatible.finalize_context().unwrap_err();
    assert!(error.message.contains("audio"));
    assert!(error.message.contains("retained conversation context"));

    let mut fork = Conversation::new("Fork", Some(&model), "");
    fork.id = "audio-fork".into();
    storage
        .fork_conversation(&conversation.id, &turn.responses[0].id, &fork)
        .unwrap();
    let copied = storage
        .attachment_path(&fork.id, &attachments[0].files[0].path)
        .unwrap();
    assert_eq!(fs::read(&copied).unwrap(), bytes);

    let mut settings = storage.load_snapshot().unwrap().settings;
    settings.current_conversation_id = Some(fork.id.clone());
    storage.save_settings(&settings).unwrap();
    let snapshot = storage.load_snapshot().unwrap();
    assert_eq!(
        snapshot.current_turns[0].user.attachments[0].audio,
        attachments[0].audio
    );

    storage.clear_conversation_context(&fork.id).unwrap();
    assert!(!copied.exists());
    assert!(storage.load_snapshot().unwrap().current_turns.is_empty());

    storage.delete_conversation(&conversation.id).unwrap();
    assert!(!source.exists());
    assert!(
        storage
            .load_snapshot()
            .unwrap()
            .conversations
            .iter()
            .all(|stored| stored.id != conversation.id)
    );
}

#[test]
fn attachment_audio_metadata_matches_the_attachment_kind() {
    let (_directory, storage) = open_storage();
    let (_provider, model) = catalog(&storage);
    let conversation = Conversation::new("Audio validation", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();

    for (id, kind, file_kind, media_type, audio, expected) in [
        (
            "missing-metadata",
            AttachmentKind::Audio,
            AttachmentFileKind::Audio,
            "audio/wav",
            None,
            "must contain audio metadata",
        ),
        (
            "zero-duration",
            AttachmentKind::Audio,
            AttachmentFileKind::Audio,
            "audio/wav",
            Some(AudioAttachmentMetadata {
                duration_ms: 0,
                source: AudioAttachmentSource::Upload,
            }),
            "greater than zero",
        ),
        (
            "audio-file-on-document",
            AttachmentKind::Document,
            AttachmentFileKind::Audio,
            "audio/wav",
            None,
            "only contain text and image",
        ),
        (
            "metadata-on-text",
            AttachmentKind::Text,
            AttachmentFileKind::Text,
            "text/plain",
            Some(AudioAttachmentMetadata {
                duration_ms: 1,
                source: AudioAttachmentSource::Voice,
            }),
            "only audio attachments",
        ),
    ] {
        let error = storage
            .store_attachments(
                &conversation.id,
                &[AttachmentDraft {
                    id: id.into(),
                    name: id.into(),
                    kind,
                    files: vec![AttachmentDraftFile {
                        name: "content.wav".into(),
                        kind: file_kind,
                        media_type: media_type.into(),
                        bytes: vec![1],
                    }],
                    audio,
                }],
            )
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
}
