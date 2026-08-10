use std::{
    fmt::Write as _,
    fs,
    io::{Cursor, Write},
    path::Path,
};

use onechat::{
    application::attachments::{load as load_attachment, validate_image},
    domain::{AttachmentDraft, AttachmentFileKind, AttachmentKind, Conversation, UserMessage},
    storage::Storage,
};
use tempfile::tempdir;

fn load(path: &Path, vision: bool) -> Result<AttachmentDraft, String> {
    load_attachment(path, vision, true)
}

#[test]
fn text_attachments_are_loaded_as_utf8() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("notes.md");
    fs::write(&path, "important context").unwrap();

    let attachment = load(&path, false).unwrap();

    assert_eq!(attachment.name, "notes.md");
    assert_eq!(attachment.kind, AttachmentKind::Text);
    assert_eq!(attachment.files.len(), 1);
    assert_eq!(attachment.files[0].name, "content.txt");
    assert_eq!(attachment.files[0].kind, AttachmentFileKind::Text);
    assert_eq!(attachment.files[0].media_type, "text/plain");
    assert_eq!(attachment.files[0].bytes, b"important context");
}

#[test]
fn binary_text_and_visual_files_without_vision_are_rejected() {
    let directory = tempdir().unwrap();
    let binary = directory.path().join("binary.txt");
    fs::write(&binary, [0xff, 0xfe]).unwrap();
    assert!(load(&binary, false).unwrap_err().contains("UTF-8"));

    let image = directory.path().join("image.png");
    fs::write(&image, b"\x89PNG\r\n\x1a\n").unwrap();
    assert!(load(&image, false).unwrap_err().contains("vision support"));

    let pdf = directory.path().join("document.pdf");
    fs::write(&pdf, b"%PDF-invalid").unwrap();
    assert!(load(&pdf, false).unwrap_err().contains("vision support"));
}

#[test]
fn image_attachments_have_a_named_image_file() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("photo.jpeg");
    fs::write(&path, [0xff, 0xd8, 0xff]).unwrap();

    let attachment = load(&path, true).unwrap();

    assert_eq!(attachment.kind, AttachmentKind::Image);
    assert!(attachment.kind.requires_vision());
    assert_eq!(attachment.files[0].name, "content.jpg");
    assert_eq!(attachment.files[0].kind, AttachmentFileKind::Image);
    assert_eq!(attachment.files[0].media_type, "image/jpeg");
}

#[test]
fn pdf_pages_have_named_image_files() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("document.pdf");
    fs::write(&path, minimal_pdf()).unwrap();

    let attachment = load(&path, true).unwrap();

    assert_eq!(attachment.kind, AttachmentKind::Pdf);
    assert!(attachment.kind.requires_vision());
    assert_eq!(attachment.files.len(), 1);
    assert_eq!(attachment.files[0].name, "page-001.png");
    assert_eq!(attachment.files[0].kind, AttachmentFileKind::Image);
    assert_eq!(attachment.files[0].media_type, "image/png");
    assert!(attachment.files[0].bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn image_signatures_must_match_the_declared_media_type() {
    assert!(validate_image(b"\x89PNG\r\n\x1a\n", "image/png").is_ok());
    assert!(validate_image(b"GIF89a", "image/gif").is_ok());
    assert!(validate_image(b"not a png", "image/png").is_err());
}

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
fn office_images_can_be_excluded_from_parsed_documents() {
    let directory = tempdir().unwrap();
    let body = format!(
        r#"<w:p><w:r><w:t>Text remains.</w:t></w:r></w:p>{}"#,
        drawing("rId001", "Architecture")
    );
    let docx_path = directory.path().join("without-images.docx");
    fs::write(
        &docx_path,
        docx(&body, vec![("diagram.png".into(), png_bytes())], Vec::new()),
    )
    .unwrap();
    let xlsx_path = directory.path().join("without-images.xlsx");
    fs::write(&xlsx_path, xlsx_fixture()).unwrap();
    let pptx_path = directory.path().join("without-images.pptx");
    fs::write(&pptx_path, pptx_fixture()).unwrap();

    for (path, expected_text) in [
        (docx_path, "Text remains."),
        (xlsx_path, "中文项目"),
        (pptx_path, "Quarterly Review"),
    ] {
        let draft = load_attachment(&path, false, false).unwrap();

        assert_eq!(draft.files.len(), 1);
        assert_eq!(draft.files[0].name, "content.md");
        let markdown = std::str::from_utf8(&draft.files[0].bytes).unwrap();
        assert!(markdown.contains(expected_text), "{markdown}");
        assert!(!markdown.contains("!["), "{markdown}");
    }
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

#[test]
fn xlsx_loads_ordered_structured_sheets_and_images() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("workbook.xlsx");
    fs::write(&path, xlsx_fixture()).unwrap();

    let draft = load(&path, false).unwrap();

    assert_eq!(draft.name, "workbook.xlsx");
    assert_eq!(draft.kind, AttachmentKind::Document);
    assert!(!draft.kind.requires_vision());
    assert_eq!(draft.files.len(), 3);
    assert_eq!(draft.files[0].name, "content.md");
    assert_eq!(draft.files[0].kind, AttachmentFileKind::Text);
    assert_eq!(draft.files[0].media_type, "text/markdown");
    assert_eq!(draft.files[1].name, "image-001.png");
    assert_eq!(draft.files[2].name, "image-002.png");
    assert!(
        draft.files[1..]
            .iter()
            .all(|file| file.kind == AttachmentFileKind::Image && file.bytes == png_bytes())
    );

    let markdown = std::str::from_utf8(&draft.files[0].bytes).unwrap();
    let overview = markdown
        .find("<!-- sheet 1: 概览 -->")
        .expect("first sheet marker");
    let details = markdown
        .find("<!-- sheet 2: Data Sheet -->")
        .expect("second sheet marker");
    assert!(overview < details, "{markdown}");
    let values = markdown
        .lines()
        .find(|line| line.contains("中文项目"))
        .expect("formatted values row");
    assert_eq!(
        &markdown_cells(values)[..3],
        ["中文项目", "42", "2024-01-01"]
    );
    assert!(markdown.contains("Second sheet / 第二页"), "{markdown}");
    assert!(
        markdown.contains("[OneChat site](https://example.com)"),
        "{markdown}"
    );
    assert!(markdown.contains("重要批注"), "{markdown}");
    assert!(!markdown.contains("#VALUE"), "{markdown}");
    assert!(markdown.contains("![image](image-001.png)"), "{markdown}");
    assert!(
        markdown.contains("![Sales chart](image-002.png)"),
        "{markdown}"
    );

    let merged = markdown
        .lines()
        .find(|line| line.contains("Group A"))
        .expect("merged header row");
    assert_eq!(markdown_cells(merged), ["Group A", "", "Metadata", ""]);
    let sparse = markdown
        .lines()
        .find(|line| line.contains("Sparse"))
        .expect("sparse row");
    assert_eq!(markdown_cells(sparse), ["Name", "Value", "", "Sparse"]);
}

#[test]
fn xlsx_formula_uses_only_the_saved_cached_value() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("formula.xlsx");
    fs::write(&path, xlsx_fixture()).unwrap();

    let draft = load(&path, false).unwrap();
    let markdown = std::str::from_utf8(&draft.files[0].bytes).unwrap();
    let formula_row = markdown
        .lines()
        .find(|line| line.contains("Cached formula"))
        .expect("formula row");
    assert!(formula_row.contains("43"), "{markdown}");
    assert!(!markdown.contains("SUM(B3,1)"), "{markdown}");
}

#[test]
fn xlsx_markdown_is_always_sent_and_images_follow_vision_support() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("message.xlsx");
    fs::write(&path, xlsx_fixture()).unwrap();
    let draft = load(&path, false).unwrap();

    let storage = Storage::open(
        directory.path().join("config/settings.jsonc"),
        directory.path().join("state"),
    )
    .unwrap();
    let conversation = Conversation::new("XLSX", None, "");
    storage.insert_conversation(&conversation).unwrap();
    let attachments = storage
        .store_attachments(&conversation.id, &[draft])
        .unwrap();
    let user = UserMessage::new("Analyze", attachments);

    let text = serde_json::to_string(
        &storage
            .message_for_user(&conversation.id, &user, false)
            .unwrap(),
    )
    .unwrap();
    assert!(text.contains("<!-- sheet 1: 概览 -->"), "{text}");
    assert!(text.contains("![image](image-001.png)"), "{text}");
    assert!(text.contains("![Sales chart](image-002.png)"), "{text}");
    assert!(!text.contains("Embedded image from"), "{text}");
    assert!(!text.contains("iVBORw0KGgo="), "{text}");

    let visual = serde_json::to_string(
        &storage
            .message_for_user(&conversation.id, &user, true)
            .unwrap(),
    )
    .unwrap();
    assert!(
        visual.contains("Embedded image from message.xlsx: image-001.png"),
        "{visual}"
    );
    assert!(
        visual.contains("Embedded image from message.xlsx: image-002.png"),
        "{visual}"
    );
    assert!(visual.contains("iVBORw0KGgo="), "{visual}");
}

#[test]
fn xlsx_missing_workbook_and_oversized_markdown_are_rejected() {
    let directory = tempdir().unwrap();
    let missing = directory.path().join("missing-workbook.xlsx");
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
    assert!(error.starts_with("Invalid XLSX:"), "{error}");
    assert!(error.contains("xl/workbook.xml"), "{error}");

    let oversized = directory.path().join("oversized.xlsx");
    let text = "x".repeat(1024 * 1024 + 1);
    let sheet = format!(
        r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>{text}</t></is></c></row></sheetData></worksheet>"#
    );
    fs::write(&oversized, xlsx(&[("Large", sheet)], "", Vec::new())).unwrap();
    let error = load(&oversized, false).unwrap_err();
    assert!(
        error.starts_with("Extracted Markdown too large:"),
        "{error}"
    );
    assert!(error.contains("1 MiB extracted Markdown limit"), "{error}");
}

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
fn pptx_missing_presentation_and_oversized_markdown_are_rejected() {
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
    let text = "x".repeat(1024 * 1024 + 1);
    let slide = format!(
        r#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#
    );
    fs::write(&oversized, pptx(&[slide], vec![String::new()], Vec::new())).unwrap();
    let error = load(&oversized, false).unwrap_err();
    assert!(
        error.starts_with("Extracted Markdown too large:"),
        "{error}"
    );
    assert!(error.contains("1 MiB extracted Markdown limit"), "{error}");
}

#[test]
fn unsupported_legacy_and_macro_office_formats_are_rejected() {
    let directory = tempdir().unwrap();
    for (extension, target) in [
        ("doc", "docx"),
        ("docm", "docx"),
        ("xls", "xlsx"),
        ("xlsm", "xlsx"),
        ("ppt", "pptx"),
        ("pptm", "pptx"),
    ] {
        let path = directory.path().join(format!("document.{extension}"));
        fs::write(&path, b"plain text").unwrap();
        let error = load(&path, false).unwrap_err();
        assert!(error.starts_with("Unsupported Office format:"), "{error}");
        assert!(
            error.contains(&format!("convert it to .{target}")),
            "{error}"
        );
    }

    let text = directory.path().join("document.xlsb");
    fs::write(&text, b"plain text").unwrap();
    assert_eq!(load(&text, false).unwrap().kind, AttachmentKind::Text);
}

#[test]
fn invalid_encrypted_and_disguised_office_files_are_rejected_by_expected_format() {
    let directory = tempdir().unwrap();

    for (extension, label) in [("docx", "DOCX"), ("xlsx", "XLSX"), ("pptx", "PPTX")] {
        let damaged = directory.path().join(format!("damaged.{extension}"));
        fs::write(&damaged, b"not a zip").unwrap();
        let error = load(&damaged, false).unwrap_err();
        assert!(error.starts_with(&format!("Invalid {label}:")), "{error}");
        assert!(error.contains("valid ZIP"), "{error}");

        let disguised = directory.path().join(format!("disguised.{extension}"));
        fs::write(
            &disguised,
            zip_entries(vec![("notes.txt".into(), b"hello".to_vec())]),
        )
        .unwrap();
        let error = load(&disguised, false).unwrap_err();
        assert!(error.starts_with(&format!("Invalid {label}:")), "{error}");
        assert!(error.contains("required OOXML entry"), "{error}");

        let encrypted = directory.path().join(format!("encrypted.{extension}"));
        fs::write(&encrypted, b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1payload").unwrap();
        let error = load(&encrypted, false).unwrap_err();
        assert!(error.starts_with(&format!("Encrypted {label}:")), "{error}");

        let encrypted_entry = directory
            .path()
            .join(format!("encrypted-entry.{extension}"));
        let mut encrypted_bytes = office_package(extension, Vec::new());
        set_central_flags(&mut encrypted_bytes, 1);
        fs::write(&encrypted_entry, encrypted_bytes).unwrap();
        let error = load(&encrypted_entry, false).unwrap_err();
        assert!(error.starts_with(&format!("Encrypted {label}:")), "{error}");
    }

    let unparseable = directory.path().join("unparseable.docx");
    fs::write(&unparseable, docx("<w:p>", Vec::new(), Vec::new())).unwrap();
    let error = load(&unparseable, false).unwrap_err();
    assert!(error.starts_with("Could not parse DOCX"), "{error}");
}

#[test]
fn mismatched_office_extension_is_rejected_as_the_expected_format() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("word-content.xlsx");
    fs::write(
        &path,
        docx(
            "<w:p><w:r><w:t>Text</w:t></w:r></w:p>",
            Vec::new(),
            vec![("xl/workbook.xml".into(), b"<workbook/>".to_vec())],
        ),
    )
    .unwrap();

    let error = load(&path, false).unwrap_err();
    assert!(error.starts_with("Invalid XLSX:"), "{error}");
    assert!(error.contains("contains DOCX content"), "{error}");
}

#[test]
fn office_archive_limits_are_checked_from_central_directory() {
    let directory = tempdir().unwrap();

    for (extension, label) in [("docx", "DOCX"), ("xlsx", "XLSX"), ("pptx", "PPTX")] {
        let too_many = directory.path().join(format!("too-many.{extension}"));
        let extras = (0..4094)
            .map(|index| (format!("extra/{index}"), Vec::new()))
            .collect();
        fs::write(&too_many, office_package(extension, extras)).unwrap();
        let error = load(&too_many, false).unwrap_err();
        assert!(
            error.starts_with(&format!("Unsafe {label} archive:")),
            "{error}"
        );
        assert!(error.contains("too many ZIP entries"), "{error}");

        let large_entry = directory.path().join(format!("large-entry.{extension}"));
        let mut bytes = office_package(extension, Vec::new());
        set_central_uncompressed_size(&mut bytes, 32 * 1024 * 1024 + 1);
        fs::write(&large_entry, bytes).unwrap();
        let error = load(&large_entry, false).unwrap_err();
        assert!(
            error.starts_with(&format!("Unsafe {label} archive:")),
            "{error}"
        );
        assert!(error.contains("larger than 32 MiB"), "{error}");

        let large_total = directory.path().join(format!("large-total.{extension}"));
        let extras = (0..6)
            .map(|index| (format!("extra/{index}"), Vec::new()))
            .collect();
        let mut bytes = office_package(extension, extras);
        set_central_uncompressed_size(&mut bytes, 32 * 1024 * 1024);
        fs::write(&large_total, bytes).unwrap();
        let error = load(&large_total, false).unwrap_err();
        assert!(
            error.starts_with(&format!("Unsafe {label} archive:")),
            "{error}"
        );
        assert!(error.contains("256 MiB uncompressed ZIP limit"), "{error}");

        let source = directory.path().join(format!("source.{extension}"));
        fs::write(&source, vec![0; 20 * 1024 * 1024 + 1]).unwrap();
        let error = load(&source, false).unwrap_err();
        assert!(error.starts_with("Source file too large:"), "{error}");
        assert!(error.contains(&format!("20 MiB {label} limit")), "{error}");
        fs::remove_file(source).unwrap();
    }
}

#[test]
fn empty_and_oversized_markdown_docx_are_rejected() {
    let directory = tempdir().unwrap();
    let empty = directory.path().join("empty.docx");
    fs::write(&empty, docx("", Vec::new(), Vec::new())).unwrap();
    let error = load(&empty, false).unwrap_err();
    assert!(error.contains("empty DOCX"), "{error}");
    assert!(error.contains("no readable content"), "{error}");

    let oversized = directory.path().join("oversized.docx");
    let text = "x".repeat(1024 * 1024 + 1);
    let body = format!("<w:p><w:r><w:t>{text}</w:t></w:r></w:p>");
    fs::write(&oversized, docx(&body, Vec::new(), Vec::new())).unwrap();
    let error = load(&oversized, false).unwrap_err();
    assert!(
        error.starts_with("Extracted Markdown too large:"),
        "{error}"
    );
    assert!(error.contains("extracted Markdown limit"), "{error}");
}

fn png_bytes() -> Vec<u8> {
    b"\x89PNG\r\n\x1a\n".to_vec()
}

fn drawing(resource_id: &str, alt: &str) -> String {
    format!(
        r#"<w:p><w:r><w:drawing><wp:inline><wp:docPr descr="{alt}"/><a:graphic><a:graphicData><a:blip r:embed="{resource_id}"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    )
}

fn docx(body: &str, media: Vec<(String, Vec<u8>)>, extras: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let relationships = media
        .iter()
        .enumerate()
        .map(|(index, (name, _))| {
            format!(
                r#"<Relationship Id="rId{:03}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/{name}"/>"#,
                index + 1
            )
        })
        .collect::<String>();
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"><w:body>{body}</w:body></w:document>"#
    );
    let document_relationships = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{relationships}</Relationships>"#
    );
    let mut entries = vec![
        (
            "[Content_Types].xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="jpg" ContentType="image/jpeg"/><Default Extension="svg" ContentType="image/svg+xml"/><Default Extension="bmp" ContentType="image/bmp"/><Default Extension="tiff" ContentType="image/tiff"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_vec(),
        ),
        (
            "_rels/.rels".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_vec(),
        ),
        (
            "word/_rels/document.xml.rels".into(),
            document_relationships.into_bytes(),
        ),
        (
            "word/numbering.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?><w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#.to_vec(),
        ),
        (
            "word/styles.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="Heading 1"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr></w:style></w:styles>"#.to_vec(),
        ),
        ("word/document.xml".into(), document.into_bytes()),
    ];
    entries.extend(
        media
            .into_iter()
            .map(|(name, bytes)| (format!("word/media/{name}"), bytes)),
    );
    entries.extend(extras);
    zip_entries(entries)
}

fn markdown_cells(line: &str) -> Vec<&str> {
    line.trim_matches('|').split('|').map(str::trim).collect()
}

fn xlsx_fixture() -> Vec<u8> {
    let overview = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
           xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>Group A</t></is></c>
      <c r="C1" t="inlineStr"><is><t>Metadata</t></is></c>
    </row>
    <row r="2">
      <c r="A2" t="inlineStr"><is><t>Name</t></is></c>
      <c r="B2" t="inlineStr"><is><t>Value</t></is></c>
      <c r="D2" t="inlineStr"><is><t>Sparse</t></is></c>
    </row>
    <row r="3">
      <c r="A3" t="inlineStr"><is><t>中文项目</t></is></c>
      <c r="B3"><v>42</v></c>
      <c r="C3" s="1"><v>45292</v></c>
    </row>
    <row r="4">
      <c r="A4" t="inlineStr"><is><t>Website</t></is></c>
      <c r="B4" t="inlineStr"><is><t>OneChat site</t></is></c>
    </row>
    <row r="5">
      <c r="A5" t="inlineStr"><is><t>Cached formula</t></is></c>
      <c r="B5"><f>SUM(B3,1)</f><v>43</v></c>
      <c r="C5" t="inlineStr"><is><t>Needs review</t></is></c>
    </row>
    <row r="6">
      <c r="A6" t="inlineStr"><is><t>In-cell image</t></is></c>
      <c r="B6" t="e" vm="1"><v>#VALUE!</v></c>
    </row>
  </sheetData>
  <mergeCells count="2"><mergeCell ref="A1:B1"/><mergeCell ref="C1:D1"/></mergeCells>
  <hyperlinks><hyperlink ref="B4" r:id="rIdLink"/></hyperlinks>
  <drawing r:id="rIdDrawing"/>
</worksheet>"#
        .to_string();
    let details = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Name</t></is></c><c r="B1" t="inlineStr"><is><t>Value</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>Second sheet / 第二页</t></is></c><c r="B2"><v>7</v></c></row>
  </sheetData>
</worksheet>"#
        .to_string();
    let extras = vec![
        (
            "xl/styles.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="14"/></cellXfs>
</styleSheet>"#
                .to_vec(),
        ),
        (
            "xl/worksheets/_rels/sheet1.xml.rels".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com" TargetMode="External"/>
  <Relationship Id="rIdComment" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="../comments1.xml"/>
  <Relationship Id="rIdDrawing" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/>
</Relationships>"#
                .to_vec(),
        ),
        (
            "xl/comments1.xml".into(),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <authors><author>OneChat</author></authors>
  <commentList><comment ref="C5" authorId="0"><text><t>重要批注</t></text></comment></commentList>
</comments>"#
                .as_bytes()
                .to_vec(),
        ),
        (
            "xl/drawings/drawing1.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"
          xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <xdr:twoCellAnchor><xdr:pic>
    <xdr:nvPicPr><xdr:cNvPr id="1" name="Sales chart"/></xdr:nvPicPr>
    <xdr:blipFill><a:blip r:embed="rIdImage"/></xdr:blipFill>
    <xdr:spPr><a:xfrm><a:ext cx="914400" cy="914400"/></a:xfrm></xdr:spPr>
  </xdr:pic><xdr:clientData/></xdr:twoCellAnchor>
</xdr:wsDr>"#
                .to_vec(),
        ),
        (
            "xl/drawings/_rels/drawing1.xml.rels".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/chart.png"/>
</Relationships>"#
                .to_vec(),
        ),
        (
            "xl/metadata.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<metadata xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:xlrd="http://schemas.microsoft.com/office/spreadsheetml/2017/richdata">
  <metadataTypes count="1"><metadataType name="XLRICHVALUE"/></metadataTypes>
  <futureMetadata name="XLRICHVALUE" count="1"><bk><extLst><ext><xlrd:rvb i="0"/></ext></extLst></bk></futureMetadata>
  <valueMetadata count="1"><bk><rc t="1" v="0"/></bk></valueMetadata>
</metadata>"#
                .to_vec(),
        ),
        (
            "xl/richData/rdrichvaluestructure.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<rvStructures xmlns="http://schemas.microsoft.com/office/spreadsheetml/2017/richdata" count="1">
  <s t="_localImage"><k n="_rvRel:LocalImageIdentifier" t="i"/></s>
</rvStructures>"#
                .to_vec(),
        ),
        (
            "xl/richData/rdrichvalue.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<rvData xmlns="http://schemas.microsoft.com/office/spreadsheetml/2017/richdata" count="1">
  <rv s="0"><v>0</v></rv>
</rvData>"#
                .to_vec(),
        ),
        (
            "xl/richData/richValueRel.xml".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<richValueRels xmlns="http://schemas.microsoft.com/office/spreadsheetml/2022/richvaluerel"
               xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <rel r:id="rIdCellImage"/>
</richValueRels>"#
                .to_vec(),
        ),
        (
            "xl/richData/_rels/richValueRel.xml.rels".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdCellImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/cell.png"/>
</Relationships>"#
                .to_vec(),
        ),
        ("xl/media/cell.png".into(), png_bytes()),
        ("xl/media/chart.png".into(), png_bytes()),
    ];
    let rich_value_relationships = r#"
  <Relationship Id="rIdMetadata" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sheetMetadata" Target="metadata.xml"/>
  <Relationship Id="rIdRichValue" Type="http://schemas.microsoft.com/office/2017/06/relationships/rdRichValue" Target="richData/rdrichvalue.xml"/>
  <Relationship Id="rIdRichStructure" Type="http://schemas.microsoft.com/office/2017/06/relationships/rdRichValueStructure" Target="richData/rdrichvaluestructure.xml"/>
  <Relationship Id="rIdRichRel" Type="http://schemas.microsoft.com/office/2022/10/relationships/richValueRel" Target="richData/richValueRel.xml"/>"#;

    xlsx(
        &[("概览", overview), ("Data Sheet", details)],
        rich_value_relationships,
        extras,
    )
}

fn xlsx(
    sheets: &[(&str, String)],
    extra_workbook_relationships: &str,
    extras: Vec<(String, Vec<u8>)>,
) -> Vec<u8> {
    let sheet_types = (1..=sheets.len())
        .map(|index| {
            format!(
                r#"<Override PartName="/xl/worksheets/sheet{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
            )
        })
        .collect::<String>();
    let content_types = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  {sheet_types}
</Types>"#
    );
    let workbook_sheets = sheets
        .iter()
        .enumerate()
        .map(|(index, (name, _))| {
            let id = index + 1;
            format!(r#"<sheet name="{name}" sheetId="{id}" r:id="rId{id}"/>"#)
        })
        .collect::<String>();
    let workbook = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>{workbook_sheets}</sheets>
</workbook>"#
    );
    let workbook_relationships = sheets
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let id = index + 1;
            format!(
                r#"<Relationship Id="rId{id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{id}.xml"/>"#
            )
        })
        .collect::<String>();
    let workbook_relationships = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  {workbook_relationships}
  {extra_workbook_relationships}
</Relationships>"#
    );

    let mut entries = vec![
        ("[Content_Types].xml".into(), content_types.into_bytes()),
        (
            "_rels/.rels".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#
                .to_vec(),
        ),
        ("xl/workbook.xml".into(), workbook.into_bytes()),
        (
            "xl/_rels/workbook.xml.rels".into(),
            workbook_relationships.into_bytes(),
        ),
    ];
    entries.extend(sheets.iter().enumerate().map(|(index, (_, xml))| {
        (
            format!("xl/worksheets/sheet{}.xml", index + 1),
            xml.as_bytes().to_vec(),
        )
    }));
    entries.extend(extras);
    zip_entries(entries)
}

fn pptx_fixture() -> Vec<u8> {
    let slide1 = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
  <p:cSld><p:spTree>
    <p:sp>
      <p:nvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>
      <p:txBody><a:p><a:r><a:t>Quarterly Review / 季度回顾</a:t></a:r></a:p></p:txBody>
    </p:sp>
    <p:sp>
      <p:txBody>
        <a:p><a:r><a:t>English and 中文 presentation text.</a:t></a:r></a:p>
        <a:p><a:pPr><a:buChar char="•"/></a:pPr><a:r><a:t>First bullet</a:t></a:r></a:p>
        <a:p><a:pPr><a:buChar char="•"/></a:pPr><a:r><a:t>Second bullet</a:t></a:r></a:p>
        <a:p><a:r><a:rPr><a:hlinkClick r:id="rIdLink"/></a:rPr><a:t>Project site</a:t></a:r></a:p>
      </p:txBody>
    </p:sp>
    <p:sp><p:nvSpPr><p:nvPr><p:ph idx="7"/></p:nvPr></p:nvSpPr></p:sp>
    <p:graphicFrame><a:graphic><a:graphicData><a:tbl>
      <a:tr><a:tc><a:txBody><a:p><a:r><a:t>Metric</a:t></a:r></a:p></a:txBody></a:tc><a:tc><a:txBody><a:p><a:r><a:t>Value</a:t></a:r></a:p></a:txBody></a:tc></a:tr>
      <a:tr><a:tc><a:txBody><a:p><a:r><a:t>Revenue</a:t></a:r></a:p></a:txBody></a:tc><a:tc><a:txBody><a:p><a:r><a:t>42</a:t></a:r></a:p></a:txBody></a:tc></a:tr>
    </a:tbl></a:graphicData></a:graphic></p:graphicFrame>
    <p:graphicFrame><a:graphic><a:graphicData><c:chart r:id="rIdChart"/></a:graphicData></a:graphic></p:graphicFrame>
    <p:pic>
      <p:nvPicPr><p:cNvPr id="1" name="Product screenshot"/></p:nvPicPr>
      <p:blipFill><a:blip r:embed="rIdImage"/></p:blipFill>
      <p:spPr><a:xfrm><a:ext cx="914400" cy="914400"/></a:xfrm></p:spPr>
    </p:pic>
  </p:spTree></p:cSld>
</p:sld>"#
        .to_string();
    let slide2 = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>Second slide / 第二页</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld>
</p:sld>"#
        .to_string();
    let slide1_relationships = r#"
  <Relationship Id="rIdLink" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/ppt" TargetMode="External"/>
  <Relationship Id="rIdChart" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="../charts/chart1.xml"/>
  <Relationship Id="rIdImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/screenshot.png"/>
  <Relationship Id="rIdLayout" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>"#
        .to_string();
    let notes = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:notes xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
         xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>Speaker notes: verify revenue assumptions.</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld>
</p:notes>"#;
    let layout = r#"<?xml version="1.0" encoding="UTF-8"?>
<p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
             xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:cSld><p:spTree><p:sp>
    <p:nvSpPr><p:nvPr><p:ph idx="7"/></p:nvPr></p:nvSpPr>
    <p:txBody><a:p><a:r><a:t>Inherited layout text</a:t></a:r></a:p></p:txBody>
  </p:sp></p:spTree></p:cSld>
</p:sldLayout>"#;
    let chart = r#"<?xml version="1.0" encoding="UTF-8"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"
              xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <c:chart>
    <c:title><c:tx><c:rich><a:p><a:r><a:t>Revenue Growth</a:t></a:r></a:p></c:rich></c:tx></c:title>
    <c:plotArea><c:barChart>
      <c:ser>
        <c:tx><c:strRef><c:strCache><c:pt idx="0"><c:v>2024</c:v></c:pt></c:strCache></c:strRef></c:tx>
        <c:cat><c:strRef><c:strCache><c:pt idx="0"><c:v>Q1</c:v></c:pt><c:pt idx="1"><c:v>Q2</c:v></c:pt></c:strCache></c:strRef></c:cat>
        <c:val><c:numRef><c:numCache><c:pt idx="0"><c:v>100</c:v></c:pt><c:pt idx="1"><c:v>150</c:v></c:pt></c:numCache></c:numRef></c:val>
      </c:ser>
      <c:ser>
        <c:tx><c:strRef><c:strCache><c:pt idx="0"><c:v>2025</c:v></c:pt></c:strCache></c:strRef></c:tx>
        <c:cat><c:strRef><c:strCache><c:pt idx="0"><c:v>Q1</c:v></c:pt><c:pt idx="1"><c:v>Q2</c:v></c:pt></c:strCache></c:strRef></c:cat>
        <c:val><c:numRef><c:numCache><c:pt idx="0"><c:v>120</c:v></c:pt><c:pt idx="1"><c:v>180</c:v></c:pt></c:numCache></c:numRef></c:val>
      </c:ser>
    </c:barChart></c:plotArea>
  </c:chart>
</c:chartSpace>"#;
    let extras = vec![
        (
            "ppt/notesSlides/notesSlide1.xml".into(),
            notes.as_bytes().to_vec(),
        ),
        (
            "ppt/slideLayouts/slideLayout1.xml".into(),
            layout.as_bytes().to_vec(),
        ),
        ("ppt/charts/chart1.xml".into(), chart.as_bytes().to_vec()),
        ("ppt/media/screenshot.png".into(), png_bytes()),
    ];

    pptx(
        &[slide1, slide2],
        vec![slide1_relationships, String::new()],
        extras,
    )
}

fn pptx(
    slides: &[String],
    slide_relationships: Vec<String>,
    extras: Vec<(String, Vec<u8>)>,
) -> Vec<u8> {
    assert_eq!(slides.len(), slide_relationships.len());
    let slide_types = (1..=slides.len())
        .map(|index| {
            format!(
                r#"<Override PartName="/ppt/slides/slide{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
            )
        })
        .collect::<String>();
    let content_types = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  {slide_types}
</Types>"#
    );
    let slide_ids = (1..=slides.len())
        .map(|index| format!(r#"<p:sldId id="{}" r:id="rId{index}"/>"#, 255 + index))
        .collect::<String>();
    let presentation = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldIdLst>{slide_ids}</p:sldIdLst>
</p:presentation>"#
    );
    let presentation_relationships = (1..=slides.len())
        .map(|index| {
            format!(
                r#"<Relationship Id="rId{index}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{index}.xml"/>"#
            )
        })
        .collect::<String>();
    let presentation_relationships = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  {presentation_relationships}
</Relationships>"#
    );

    let mut entries = vec![
        ("[Content_Types].xml".into(), content_types.into_bytes()),
        (
            "_rels/.rels".into(),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#
                .to_vec(),
        ),
        ("ppt/presentation.xml".into(), presentation.into_bytes()),
        (
            "ppt/_rels/presentation.xml.rels".into(),
            presentation_relationships.into_bytes(),
        ),
    ];
    for (index, (slide, relationships)) in slides.iter().zip(slide_relationships).enumerate() {
        let number = index + 1;
        entries.push((
            format!("ppt/slides/slide{number}.xml"),
            slide.as_bytes().to_vec(),
        ));
        entries.push((
            format!("ppt/slides/_rels/slide{number}.xml.rels"),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  {relationships}
</Relationships>"#
            )
            .into_bytes(),
        ));
    }
    entries.extend(extras);
    zip_entries(entries)
}

fn office_package(extension: &str, extras: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let (main_entry, content_type) = match extension {
        "docx" => (
            "word/document.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
        ),
        "xlsx" => (
            "xl/workbook.xml",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
        ),
        "pptx" => (
            "ppt/presentation.xml",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
        ),
        _ => panic!("unsupported test Office extension: {extension}"),
    };
    let content_types = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/{main_entry}" ContentType="{content_type}"/></Types>"#
    );
    let relationships = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="{main_entry}"/></Relationships>"#
    );
    let mut entries = vec![
        ("[Content_Types].xml".into(), content_types.into_bytes()),
        ("_rels/.rels".into(), relationships.into_bytes()),
        (main_entry.into(), b"<root/>".to_vec()),
    ];
    entries.extend(extras);
    zip_entries(entries)
}

fn zip_entries(entries: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in entries {
        zip.start_file(name, options).unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn set_central_flags(bytes: &mut [u8], flags: u16) {
    for index in 0..bytes.len().saturating_sub(10) {
        if bytes[index..].starts_with(b"PK\x01\x02") {
            bytes[index + 8..index + 10].copy_from_slice(&flags.to_le_bytes());
        }
    }
}

fn set_central_uncompressed_size(bytes: &mut [u8], size: u32) {
    for index in 0..bytes.len().saturating_sub(28) {
        if bytes[index..].starts_with(b"PK\x01\x02") {
            bytes[index + 24..index + 28].copy_from_slice(&size.to_le_bytes());
        }
    }
}

fn minimal_pdf() -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Resources << >> /Contents 4 0 R >>",
        "<< /Length 0 >>\nstream\n\nendstream",
    ];
    let mut pdf = "%PDF-1.4\n".to_string();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        writeln!(&mut pdf, "{} 0 obj\n{object}\nendobj", index + 1).unwrap();
    }
    let xref = pdf.len();
    write!(&mut pdf, "xref\n0 5\n0000000000 65535 f \n").unwrap();
    for offset in offsets {
        writeln!(&mut pdf, "{offset:010} 00000 n ").unwrap();
    }
    write!(
        &mut pdf,
        "trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    )
    .unwrap();
    pdf.into_bytes()
}
