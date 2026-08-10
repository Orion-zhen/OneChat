use super::*;

#[test]
fn docx_loads_structured_markdown_and_images_without_vision() {
    let directory = tempdir().unwrap();
    let body = format!(
        r#"
<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>项目报告</w:t></w:r></w:p>
<w:p><w:r><w:t>English and 中文 paragraph.</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>First item</w:t></w:r></w:p>
<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Value</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>OneChat</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Ready</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
{}"#,
        drawing("rId001", "Architecture")
    );
    let path = directory.path().join("report.docx");
    fs::write(
        &path,
        docx(&body, vec![("chart.png".into(), png_bytes())], Vec::new()),
    )
    .unwrap();

    let draft = load(&path, false).unwrap();

    assert_eq!(draft.name, "report.docx");
    assert_eq!(draft.kind, AttachmentKind::Document);
    assert!(!draft.kind.requires_vision());
    assert_eq!(draft.files.len(), 2);
    assert_eq!(draft.files[0].name, "content.md");
    assert_eq!(draft.files[0].kind, AttachmentFileKind::Text);
    assert_eq!(draft.files[0].media_type, "text/markdown");
    let markdown = std::str::from_utf8(&draft.files[0].bytes).unwrap();
    assert!(!markdown.contains("<!-- sheet"), "{markdown}");
    assert!(!markdown.contains("<!-- slide"), "{markdown}");
    assert!(markdown.contains("# 项目报告"), "{markdown}");
    assert!(
        markdown.contains("English and 中文 paragraph."),
        "{markdown}"
    );
    assert!(markdown.contains("1. First item"), "{markdown}");
    assert!(markdown.contains("| Name | Value |"), "{markdown}");
    assert!(
        markdown.contains("![Architecture](image-001.png)"),
        "{markdown}"
    );
    assert_eq!(draft.files[1].name, "image-001.png");
    assert_eq!(draft.files[1].kind, AttachmentFileKind::Image);
    assert_eq!(draft.files[1].bytes, png_bytes());

    let storage = Storage::open(
        directory.path().join("config/settings.jsonc"),
        directory.path().join("state"),
    )
    .unwrap();
    let conversation = Conversation::new("DOCX", None, "");
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
    assert!(text.contains("![Architecture](image-001.png)"));
    assert!(!text.contains("Embedded image from"));
    let visual = serde_json::to_string(
        &storage
            .message_for_user(&conversation.id, &user, true)
            .unwrap(),
    )
    .unwrap();
    assert!(visual.contains("Embedded image from report.docx: image-001.png"));
    assert!(visual.contains("iVBORw0KGgo="));
}

#[test]
fn docx_images_convert_or_degrade_to_named_placeholders() {
    let directory = tempdir().unwrap();
    let mut bmp = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut bmp, image::ImageFormat::Bmp)
        .unwrap();
    let mut tiff = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut tiff, image::ImageFormat::Tiff)
        .unwrap();
    let oversized = [png_bytes(), vec![0; 10 * 1024 * 1024]].concat();
    let media = vec![
        ("valid.png".into(), png_bytes()),
        ("broken.jpg".into(), b"broken jpeg".to_vec()),
        ("vector.svg".into(), b"<svg/>".to_vec()),
        ("large.png".into(), oversized),
        ("bitmap.bmp".into(), bmp.into_inner()),
        ("scan.tiff".into(), tiff.into_inner()),
    ];
    let body = (1..=media.len())
        .map(|index| drawing(&format!("rId{index:03}"), &format!("Image {index}")))
        .collect::<String>();
    let path = directory.path().join("media.docx");
    fs::write(&path, docx(&body, media, Vec::new())).unwrap();

    let draft = load(&path, false).unwrap();
    let markdown = std::str::from_utf8(&draft.files[0].bytes).unwrap();
    for expected in [
        "image-001.png",
        "image-002.jpg",
        "image-003.svg",
        "image-004.png",
        "image-005.png",
        "image-006.png",
    ] {
        assert!(
            markdown.contains(expected),
            "missing {expected} in {markdown}"
        );
    }
    let images = &draft.files[1..];
    assert_eq!(
        images
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>(),
        ["image-001.png", "image-005.png", "image-006.png"]
    );
    assert!(images.iter().all(|file| file.media_type == "image/png"));
    assert!(
        images
            .iter()
            .all(|file| file.bytes.starts_with(b"\x89PNG\r\n\x1a\n"))
    );
}

#[test]
fn docx_image_count_and_total_size_quotas_only_drop_image_files() {
    let directory = tempdir().unwrap();
    let tiny_media = (1..=21)
        .map(|index| (format!("image-{index}.png"), png_bytes()))
        .collect::<Vec<_>>();
    let tiny_body = (1..=21)
        .map(|index| drawing(&format!("rId{index:03}"), "Image"))
        .collect::<String>();
    let count_path = directory.path().join("many.docx");
    fs::write(&count_path, docx(&tiny_body, tiny_media, Vec::new())).unwrap();
    let count = load(&count_path, false).unwrap();
    assert_eq!(count.files.len(), 21);
    let markdown = std::str::from_utf8(&count.files[0].bytes).unwrap();
    assert!(markdown.contains("image-021.png"));

    let large_image = [png_bytes(), vec![0; 9 * 1024 * 1024]].concat();
    let large_media = (1..=6)
        .map(|index| (format!("large-{index}.png"), large_image.clone()))
        .collect::<Vec<_>>();
    let large_body = (1..=6)
        .map(|index| drawing(&format!("rId{index:03}"), "Large"))
        .collect::<String>();
    let total_path = directory.path().join("total.docx");
    fs::write(&total_path, docx(&large_body, large_media, Vec::new())).unwrap();
    let total = load(&total_path, false).unwrap();
    assert_eq!(total.files.len(), 6, "content.md plus five images");
    assert!(
        std::str::from_utf8(&total.files[0].bytes)
            .unwrap()
            .contains("image-006.png")
    );
}
