use std::fs;

use onechat::{
    application::attachments::{load, validate_image},
    domain::AttachmentKind,
};
use tempfile::tempdir;

#[test]
fn text_attachments_are_loaded_as_utf8() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("notes.md");
    fs::write(&path, "important context").unwrap();

    let attachment = load(&path, false).unwrap();

    assert_eq!(attachment.name, "notes.md");
    assert_eq!(attachment.kind, AttachmentKind::Text);
    assert_eq!(attachment.files.len(), 1);
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
}

#[test]
fn image_signatures_must_match_the_declared_media_type() {
    assert!(validate_image(b"\x89PNG\r\n\x1a\n", "image/png").is_ok());
    assert!(validate_image(b"GIF89a", "image/gif").is_ok());
    assert!(validate_image(b"not a png", "image/png").is_err());
}
