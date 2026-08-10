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
                },
                AttachmentDraft {
                    id: "duplicate".into(),
                    name: "pages.pdf".into(),
                    kind: AttachmentKind::Pdf,
                    files: vec![image_file("page.png"), image_file("page.png")],
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
            }],
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("text attachment must contain exactly one text file")
    );
}
