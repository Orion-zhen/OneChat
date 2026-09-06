use super::*;

fn options(vision: bool, audio_input: bool, parse_document_images: bool) -> LoadManyOptions {
    LoadManyOptions {
        vision,
        audio_input,
        parse_document_images,
    }
}

#[test]
fn batch_loading_accepts_more_than_ten_attachments() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("notes.txt");
    fs::write(&path, "notes").unwrap();

    let attachments = load_many(vec![path; 25], options(false, false, true)).unwrap();
    assert_eq!(attachments.len(), 25);
    assert!(
        attachments
            .iter()
            .all(|draft| draft.files[0].bytes == b"notes")
    );
}

#[test]
fn batch_loading_rejects_directories_and_returns_the_first_path_error() {
    let directory = tempdir().unwrap();
    let text = directory.path().join("notes.txt");
    fs::write(&text, "notes").unwrap();

    let error = load_many(
        vec![directory.path().to_path_buf()],
        options(false, false, true),
    )
    .unwrap_err();
    assert_eq!(
        error,
        format!(
            "Folders cannot be added as attachments: {}",
            directory.path().display()
        )
    );

    let missing = directory.path().join("missing.txt");
    let error = load_many(vec![text, missing], options(false, false, true)).unwrap_err();
    assert!(error.contains("Could not read missing.txt"), "{error}");
}

#[test]
fn batch_loading_applies_vision_and_audio_capabilities() {
    let directory = tempdir().unwrap();
    let image = directory.path().join("image.png");
    fs::write(&image, png_bytes()).unwrap();
    let audio = directory.path().join("speech.mp3");
    fs::write(&audio, include_bytes!("../fixtures/audio/minimal.mp3")).unwrap();

    assert!(
        load_many(vec![image.clone()], options(false, false, true))
            .unwrap_err()
            .contains("vision support")
    );
    assert_eq!(
        load_many(vec![image], options(true, false, true)).unwrap()[0].kind,
        AttachmentKind::Image
    );
    assert!(
        load_many(vec![audio.clone()], options(false, false, true))
            .unwrap_err()
            .contains("audio support")
    );
    assert_eq!(
        load_many(vec![audio], options(false, true, true)).unwrap()[0].kind,
        AttachmentKind::Audio
    );
}

#[test]
fn batch_loading_respects_the_office_image_option() {
    let directory = tempdir().unwrap();
    let body = format!(
        r#"<w:p><w:r><w:t>Text remains.</w:t></w:r></w:p>{}"#,
        drawing("rId001", "Diagram")
    );
    let path = directory.path().join("report.docx");
    fs::write(
        &path,
        docx(&body, vec![("diagram.png".into(), png_bytes())], Vec::new()),
    )
    .unwrap();

    let without_images = load_many(vec![path.clone()], options(false, false, false)).unwrap();
    assert_eq!(without_images[0].files.len(), 1);
    let with_images = load_many(vec![path], options(false, false, true)).unwrap();
    assert_eq!(with_images[0].files.len(), 2);
    assert_eq!(with_images[0].files[1].kind, AttachmentFileKind::Image);
}
