use super::*;

#[test]
fn image_and_pdf_assets_keep_names_roles_and_message_content() {
    let (_directory, storage) = open_storage();
    let (_provider, model) = catalog(&storage);
    let conversation = Conversation::new("Visual attachments", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();

    let image_file = |name: &str| AttachmentDraftFile {
        name: name.into(),
        kind: AttachmentFileKind::Image,
        media_type: "image/png".into(),
        bytes: b"\x89PNG\r\n\x1a\n".to_vec(),
    };
    let attachments = storage
        .store_attachments(
            &conversation.id,
            &[
                AttachmentDraft {
                    id: "image".into(),
                    name: "photo.png".into(),
                    kind: AttachmentKind::Image,
                    files: vec![image_file("content.png")],
                },
                AttachmentDraft {
                    id: "pdf".into(),
                    name: "document.pdf".into(),
                    kind: AttachmentKind::Pdf,
                    files: vec![image_file("page-001.png"), image_file("page-002.png")],
                },
            ],
        )
        .unwrap();

    assert_eq!(attachments[0].files[0].name, "content.png");
    assert_eq!(attachments[0].files[0].kind, AttachmentFileKind::Image);
    assert_eq!(attachments[1].files[1].name, "page-002.png");
    assert_eq!(attachments[1].files[1].path, "attachments/pdf/page-002.png");

    let message = storage
        .message_for_user(&conversation.id, &UserMessage::new("", attachments), false)
        .unwrap();
    let json = serde_json::to_string(&message).unwrap();
    assert!(json.contains("Image attachment: photo.png"));
    assert!(json.contains("PDF attachment: document.pdf (2 pages)"));
    assert!(json.contains("Page 1"));
    assert!(json.contains("Page 2"));
}

#[test]
fn documents_send_markdown_and_conditionally_include_named_images() {
    let (_directory, storage) = open_storage();
    let (_provider, model) = catalog(&storage);
    let conversation = Conversation::new("Documents", Some(&model), "");
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
                        bytes: b"# Report\n![First](image-001.png)\n![Second](image-002.png)"
                            .to_vec(),
                    },
                    AttachmentDraftFile {
                        name: "image-002.png".into(),
                        kind: AttachmentFileKind::Image,
                        media_type: "image/png".into(),
                        bytes: b"second image".to_vec(),
                    },
                    AttachmentDraftFile {
                        name: "image-001.png".into(),
                        kind: AttachmentFileKind::Image,
                        media_type: "image/png".into(),
                        bytes: b"first image".to_vec(),
                    },
                ],
            }],
        )
        .unwrap();
    assert!(!attachments[0].kind.requires_vision());
    storage
        .store_attachments(
            &conversation.id,
            &[AttachmentDraft {
                id: "markdown-only".into(),
                name: "notes.docx".into(),
                kind: AttachmentKind::Document,
                files: vec![AttachmentDraftFile {
                    name: "content.md".into(),
                    kind: AttachmentFileKind::Text,
                    media_type: "text/markdown".into(),
                    bytes: b"No images".to_vec(),
                }],
            }],
        )
        .unwrap();

    let user = UserMessage::new("Summarize this", attachments.clone());
    let text = serde_json::to_string(
        &storage
            .message_for_user(&conversation.id, &user, false)
            .unwrap(),
    )
    .unwrap();
    assert!(text.contains("# Report"));
    assert!(text.contains("![First](image-001.png)"));
    assert!(!text.contains("Embedded image from"));
    assert!(!text.contains("Zmlyc3QgaW1hZ2U="));

    let visual = serde_json::to_string(
        &storage
            .message_for_user(&conversation.id, &user, true)
            .unwrap(),
    )
    .unwrap();
    let first = visual
        .find("Embedded image from report.docx: image-001.png")
        .unwrap();
    let second = visual
        .find("Embedded image from report.docx: image-002.png")
        .unwrap();
    assert!(first < second);
    assert!(visual.contains("Zmlyc3QgaW1hZ2U="));
    assert!(visual.contains("c2Vjb25kIGltYWdl"));
}

#[test]
fn documents_reject_invalid_shapes_and_only_require_present_included_resources() {
    let (_directory, storage) = open_storage();
    let (_provider, model) = catalog(&storage);
    let conversation = Conversation::new("Documents", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();

    let markdown = |name: &str, media_type: &str| AttachmentDraftFile {
        name: name.into(),
        kind: AttachmentFileKind::Text,
        media_type: media_type.into(),
        bytes: b"![Image](image-001.png)".to_vec(),
    };
    let image = |name: &str| AttachmentDraftFile {
        name: name.into(),
        kind: AttachmentFileKind::Image,
        media_type: "image/png".into(),
        bytes: b"image".to_vec(),
    };

    for (id, files, expected) in [
        (
            "missing-markdown",
            vec![image("image-001.png")],
            "must contain content.md",
        ),
        (
            "wrong-markdown-name",
            vec![markdown("document.md", "text/markdown")],
            "must be content.md",
        ),
        (
            "wrong-markdown-type",
            vec![markdown("content.md", "text/plain")],
            "text/markdown",
        ),
        (
            "multiple-markdown",
            vec![
                markdown("content.md", "text/markdown"),
                markdown("notes.md", "text/markdown"),
            ],
            "exactly one text file",
        ),
        (
            "duplicate-image",
            vec![
                markdown("content.md", "text/markdown"),
                image("image.png"),
                image("image.png"),
            ],
            "duplicate attachment file name",
        ),
    ] {
        let error = storage
            .store_attachments(
                &conversation.id,
                &[AttachmentDraft {
                    id: id.into(),
                    name: "invalid.docx".into(),
                    kind: AttachmentKind::Document,
                    files,
                }],
            )
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }

    let attachments = storage
        .store_attachments(
            &conversation.id,
            &[AttachmentDraft {
                id: "missing-image".into(),
                name: "missing.docx".into(),
                kind: AttachmentKind::Document,
                files: vec![
                    markdown("content.md", "text/markdown"),
                    image("image-001.png"),
                ],
            }],
        )
        .unwrap();
    let image_path = storage
        .attachment_path(&conversation.id, &attachments[0].files[1].path)
        .unwrap();
    fs::remove_file(image_path).unwrap();
    let user = UserMessage::new("", attachments);
    assert!(
        storage
            .message_for_user(&conversation.id, &user, false)
            .is_ok()
    );
    assert!(
        storage
            .message_for_user(&conversation.id, &user, true)
            .unwrap_err()
            .to_string()
            .contains("No such file")
    );

    let markdown_path = storage
        .attachment_path(&conversation.id, &user.attachments[0].files[0].path)
        .unwrap();
    fs::remove_file(markdown_path).unwrap();
    assert!(
        storage
            .message_for_user(&conversation.id, &user, false)
            .unwrap_err()
            .to_string()
            .contains("No such file")
    );
}
