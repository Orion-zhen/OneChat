use super::*;

#[test]
fn document_images_follow_each_generation_target_model() {
    let directory = tempdir().unwrap();
    let storage = Storage::open(
        directory.path().join("config/settings.jsonc"),
        directory.path().join("state"),
    )
    .unwrap();
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    storage.insert_provider(&provider).unwrap();
    let text_model = Model::new(&provider.id, "text-model", "Text Model");
    storage.insert_model(&text_model).unwrap();
    let mut vision_model = text_model.clone();
    vision_model.id = "vision-model".into();
    vision_model.remote_id = "vision-model".into();
    vision_model.display_name = "Vision Model".into();
    vision_model.capabilities.vision = true;
    storage.insert_model(&vision_model).unwrap();
    let conversation = Conversation::new("Documents", Some(&text_model), "");
    storage.insert_conversation(&conversation).unwrap();

    let attachments = storage
        .store_attachments(
            &conversation.id,
            &[AttachmentDraft {
                id: "document".into(),
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
                        bytes: b"image".to_vec(),
                    },
                ],
            }],
        )
        .unwrap();
    let user = UserMessage::new("Review", attachments);
    let text_message = |user: &UserMessage| {
        storage
            .message_for_user(&conversation.id, user, text_model.capabilities.vision)
            .map_err(|error| error.to_string())
    };
    let visual_message = |user: &UserMessage| {
        storage
            .message_for_user(&conversation.id, user, vision_model.capabilities.vision)
            .map_err(|error| error.to_string())
    };
    let assert_document_images = |prepared: &PreparedGeneration, included: bool| {
        let messages = serialized_messages(&prepared.provider_request.messages).join("\n");
        assert!(messages.contains("![Chart](image-001.png)"));
        assert_eq!(
            messages.contains("Embedded image from report.docx"),
            included
        );
        assert_eq!(messages.contains("aW1hZ2U="), included);
    };

    let text_new = PreparedGeneration::new(
        &conversation,
        &provider,
        &text_model,
        &[],
        None,
        user.clone(),
        ContextPolicy::new(HistoryLimit::Unlimited, &text_message),
    )
    .unwrap();
    let visual_new = PreparedGeneration::new(
        &conversation,
        &provider,
        &vision_model,
        &[],
        None,
        user,
        ContextPolicy::new(HistoryLimit::Unlimited, &visual_message),
    )
    .unwrap();
    assert_document_images(&text_new, false);
    assert_document_images(&visual_new, true);

    let GenerationStart::NewTurn(turn) = text_new.start else {
        panic!("expected a new turn");
    };
    let text_additional = PreparedGeneration::additional(
        &conversation,
        &provider,
        &text_model,
        std::slice::from_ref(&turn),
        &turn,
        ContextPolicy::new(HistoryLimit::Unlimited, &text_message),
    )
    .unwrap();
    let visual_additional = PreparedGeneration::additional(
        &conversation,
        &provider,
        &vision_model,
        std::slice::from_ref(&turn),
        &turn,
        ContextPolicy::new(HistoryLimit::Unlimited, &visual_message),
    )
    .unwrap();
    assert_document_images(&text_additional, false);
    assert_document_images(&visual_additional, true);

    let previous_response = turn.responses[0].clone();
    let text_regenerated = PreparedGeneration::regenerate(
        &conversation,
        &provider,
        &text_model,
        std::slice::from_ref(&turn),
        &turn,
        &previous_response,
        ContextPolicy::new(HistoryLimit::Unlimited, &text_message),
    )
    .unwrap();
    let visual_regenerated = PreparedGeneration::regenerate(
        &conversation,
        &provider,
        &vision_model,
        std::slice::from_ref(&turn),
        &turn,
        &previous_response,
        ContextPolicy::new(HistoryLimit::Unlimited, &visual_message),
    )
    .unwrap();
    assert_document_images(&text_regenerated, false);
    assert_document_images(&visual_regenerated, true);
}
