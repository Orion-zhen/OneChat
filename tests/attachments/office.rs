use super::*;

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
