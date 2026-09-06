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
fn text_attachments_over_five_mib_are_loaded() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("large.txt");
    let bytes = vec![b'x'; 5 * 1024 * 1024 + 1];
    fs::write(&path, &bytes).unwrap();

    let attachment = load(&path, false).unwrap();
    assert_eq!(attachment.files[0].bytes, bytes);
}

#[test]
fn image_attachments_over_ten_mib_are_loaded() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("large.png");
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let mut bytes = png.into_inner();
    bytes.resize(10 * 1024 * 1024 + 1, 0);
    fs::write(&path, &bytes).unwrap();

    let attachment = load(&path, true).unwrap();
    assert_eq!(attachment.files[0].bytes, bytes);
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
fn pdfs_over_twenty_pages_or_twenty_mib_are_loaded() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("large.pdf");
    for (page_count, padding_bytes) in [(21, 0), (1, 20 * 1024 * 1024)] {
        fs::write(&path, pdf_fixture(page_count, padding_bytes)).unwrap();
        let attachment = load(&path, true).unwrap();
        assert_eq!(attachment.files.len(), page_count);
        assert_eq!(
            attachment.files.last().unwrap().name,
            format!("page-{page_count:03}.png")
        );
    }
}

#[test]
fn image_signatures_must_match_the_declared_media_type() {
    assert!(validate_image(b"\x89PNG\r\n\x1a\n", "image/png").is_ok());
    assert!(validate_image(b"GIF89a", "image/gif").is_ok());
    assert!(validate_image(b"not a png", "image/png").is_err());
}
