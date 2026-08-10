use super::*;

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
