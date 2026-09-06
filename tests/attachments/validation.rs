use super::*;

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
    }
}

#[test]
fn empty_docx_is_rejected_but_markdown_over_five_mib_is_loaded() {
    let directory = tempdir().unwrap();
    let empty = directory.path().join("empty.docx");
    fs::write(&empty, docx("", Vec::new(), Vec::new())).unwrap();
    let error = load(&empty, false).unwrap_err();
    assert!(error.contains("empty DOCX"), "{error}");
    assert!(error.contains("no readable content"), "{error}");

    let oversized = directory.path().join("oversized.docx");
    let text = "x".repeat(5 * 1024 * 1024 + 1);
    let body = format!("<w:p><w:r><w:t>{text}</w:t></w:r></w:p>");
    fs::write(&oversized, docx(&body, Vec::new(), Vec::new())).unwrap();
    let attachment = load(&oversized, false).unwrap();
    let markdown = std::str::from_utf8(&attachment.files[0].bytes).unwrap();
    assert_eq!(markdown.trim(), text);
}
