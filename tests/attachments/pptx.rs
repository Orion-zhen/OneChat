use super::*;

#[test]
fn pptx_loads_ordered_slides_with_structured_content_and_images() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("slides.pptx");
    fs::write(&path, pptx_fixture()).unwrap();

    let draft = load(&path, false).unwrap();

    assert_eq!(draft.name, "slides.pptx");
    assert_eq!(draft.kind, AttachmentKind::Document);
    assert!(!draft.kind.requires_vision());
    assert_eq!(draft.files.len(), 2);
    assert_eq!(draft.files[0].name, "content.md");
    assert_eq!(draft.files[0].kind, AttachmentFileKind::Text);
    assert_eq!(draft.files[0].media_type, "text/markdown");
    assert_eq!(draft.files[1].name, "image-001.png");
    assert_eq!(draft.files[1].kind, AttachmentFileKind::Image);
    assert_eq!(draft.files[1].bytes, png_bytes());

    let markdown = std::str::from_utf8(&draft.files[0].bytes).unwrap();
    let first = markdown
        .find("<!-- slide 1: Slide 1 -->")
        .expect("first slide marker");
    let second = markdown
        .find("<!-- slide 2: Slide 2 -->")
        .expect("second slide marker");
    assert!(first < second, "{markdown}");
    assert!(
        markdown.contains("# Quarterly Review / 季度回顾"),
        "{markdown}"
    );
    assert!(markdown.contains("First bullet"), "{markdown}");
    assert!(markdown.contains("Second bullet"), "{markdown}");
    assert!(markdown.contains("| Metric | Value |"), "{markdown}");
    assert!(markdown.contains("| Revenue | 42 |"), "{markdown}");
    assert!(
        markdown.contains("[Project site](https://example.com/ppt)"),
        "{markdown}"
    );
    assert!(markdown.contains("Inherited layout text"), "{markdown}");
    assert!(markdown.contains("Second slide / 第二页"), "{markdown}");
    assert!(
        markdown.contains("![Product screenshot](image-001.png)"),
        "{markdown}"
    );
}

#[test]
fn pptx_images_without_description_do_not_use_shape_names() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("images.pptx");
    for description in ["", r#"descr="   ""#] {
        let slide = format!(
            r#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                      xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                      xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
              <p:cSld><p:spTree><p:pic>
                <p:nvPicPr><p:cNvPr id="1" name="Picture 1" {description}/></p:nvPicPr>
                <p:blipFill><a:blip r:embed="rIdImage"/></p:blipFill>
              </p:pic></p:spTree></p:cSld>
            </p:sld>"#
        );
        let relationships = r#"<Relationship Id="rIdImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image.png"/>"#;
        fs::write(
            &path,
            pptx(
                &[slide],
                vec![relationships.into()],
                vec![("ppt/media/image.png".into(), png_bytes())],
            ),
        )
        .unwrap();

        let draft = load(&path, false).unwrap();
        let markdown = std::str::from_utf8(&draft.files[0].bytes).unwrap();
        assert!(markdown.contains("![image](image-001.png)"), "{markdown}");
        assert!(!markdown.contains("Picture 1"), "{markdown}");
        assert_eq!(draft.files.len(), 2);
        assert_eq!(draft.files[1].bytes, png_bytes());
    }
}

#[test]
fn pptx_preserves_chart_cache_and_speaker_notes() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("analysis.pptx");
    fs::write(&path, pptx_fixture()).unwrap();

    let draft = load(&path, false).unwrap();
    let markdown = std::str::from_utf8(&draft.files[0].bytes).unwrap();
    assert!(markdown.contains("Category (Revenue Growth)"), "{markdown}");
    assert!(markdown.contains("| Q1 | 100 | 120 |"), "{markdown}");
    assert!(markdown.contains("| Q2 | 150 | 180 |"), "{markdown}");
    assert!(markdown.contains("> **Notes:**"), "{markdown}");
    assert!(
        markdown.contains("> Speaker notes: verify revenue assumptions."),
        "{markdown}"
    );
}

#[test]
fn pptx_markdown_is_always_sent_and_images_follow_vision_support() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("message.pptx");
    fs::write(&path, pptx_fixture()).unwrap();
    let draft = load(&path, false).unwrap();

    let storage = Storage::open(
        directory.path().join("config/settings.jsonc"),
        directory.path().join("state"),
    )
    .unwrap();
    let conversation = Conversation::new("PPTX", None, "");
    storage.insert_conversation(&conversation).unwrap();
    let attachments = storage
        .store_attachments(&conversation.id, &[draft])
        .unwrap();
    let user = UserMessage::new("Summarize", attachments);

    let text = serde_json::to_string(
        &storage
            .message_for_user(&conversation.id, &user, false)
            .unwrap(),
    )
    .unwrap();
    assert!(text.contains("<!-- slide 1: Slide 1 -->"), "{text}");
    assert!(
        text.contains("![Product screenshot](image-001.png)"),
        "{text}"
    );
    assert!(!text.contains("Embedded image from"), "{text}");
    assert!(!text.contains("iVBORw0KGgo="), "{text}");

    let visual = serde_json::to_string(
        &storage
            .message_for_user(&conversation.id, &user, true)
            .unwrap(),
    )
    .unwrap();
    assert!(
        visual.contains("Embedded image from message.pptx: image-001.png"),
        "{visual}"
    );
    assert!(visual.contains("iVBORw0KGgo="), "{visual}");
}

#[test]
fn pptx_missing_presentation_is_rejected_but_markdown_over_five_mib_is_loaded() {
    let directory = tempdir().unwrap();
    let missing = directory.path().join("missing-presentation.pptx");
    fs::write(
        &missing,
        zip_entries(vec![
            (
                "[Content_Types].xml".into(),
                br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#
                    .to_vec(),
            ),
            (
                "_rels/.rels".into(),
                br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#
                    .to_vec(),
            ),
        ]),
    )
    .unwrap();
    let error = load(&missing, false).unwrap_err();
    assert!(error.starts_with("Invalid PPTX:"), "{error}");
    assert!(error.contains("ppt/presentation.xml"), "{error}");

    let oversized = directory.path().join("oversized.pptx");
    let text = "x".repeat(5 * 1024 * 1024 + 1);
    let slide = format!(
        r#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#
    );
    fs::write(&oversized, pptx(&[slide], vec![String::new()], Vec::new())).unwrap();
    let attachment = load(&oversized, false).unwrap();
    let markdown = std::str::from_utf8(&attachment.files[0].bytes).unwrap();
    assert!(markdown.contains(&text));
}
