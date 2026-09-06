use super::*;

#[test]
fn office_source_files_over_twenty_mib_are_loaded() {
    let directory = tempdir().unwrap();
    for (extension, bytes, expected_text) in [
        (
            "docx",
            docx(
                "<w:p><w:r><w:t>Large document</w:t></w:r></w:p>",
                Vec::new(),
                Vec::new(),
            ),
            "Large document",
        ),
        ("xlsx", xlsx_fixture(), "中文项目"),
        ("pptx", pptx_fixture(), "Quarterly Review"),
    ] {
        let mut archive = zip::ZipWriter::new_append(Cursor::new(bytes)).unwrap();
        archive
            .start_file(
                "padding.bin",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        archive.write_all(&vec![0; 20 * 1024 * 1024]).unwrap();
        let bytes = archive.finish().unwrap().into_inner();
        assert!(bytes.len() > 20 * 1024 * 1024);
        let path = directory.path().join(format!("large.{extension}"));
        fs::write(&path, bytes).unwrap();

        let attachment = load(&path, false).unwrap();
        let markdown = std::str::from_utf8(&attachment.files[0].bytes).unwrap();
        assert!(markdown.contains(expected_text));
    }
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
        let draft = load_attachment(&path, false, false, false).unwrap();

        assert_eq!(draft.files.len(), 1);
        assert_eq!(draft.files[0].name, "content.md");
        let markdown = std::str::from_utf8(&draft.files[0].bytes).unwrap();
        assert!(markdown.contains(expected_text), "{markdown}");
        assert!(!markdown.contains("!["), "{markdown}");
    }
}
