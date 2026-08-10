use super::*;

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
