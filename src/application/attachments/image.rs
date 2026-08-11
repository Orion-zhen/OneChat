use std::path::Path;

use crate::domain::{
    AttachmentDraft, AttachmentDraftFile, AttachmentFileKind, AttachmentKind, new_id,
};

use super::MAX_IMAGE_BYTES;

pub(super) fn load(
    path: &Path,
    name: String,
    extension: &str,
    size: u64,
    vision: bool,
) -> Result<AttachmentDraft, String> {
    if !vision {
        return Err(format!("{name} requires a model with vision support."));
    }
    if size > MAX_IMAGE_BYTES {
        return Err(format!("{name} exceeds the 10 MiB image limit."));
    }

    let media_type = media_type(extension).expect("validated image extension");
    let bytes = std::fs::read(path).map_err(|error| format!("Could not read {name}: {error}"))?;
    validate_image(&bytes, media_type).map_err(|error| format!("Invalid image {name}: {error}"))?;

    Ok(AttachmentDraft {
        id: new_id("attachment"),
        name,
        kind: AttachmentKind::Image,
        files: vec![AttachmentDraftFile {
            name: format!("content.{}", canonical_extension(media_type)),
            kind: AttachmentFileKind::Image,
            media_type: media_type.into(),
            bytes,
        }],
        audio: None,
    })
}

pub fn validate_image(bytes: &[u8], media_type: &str) -> Result<(), &'static str> {
    let valid = match media_type {
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        _ => false,
    };
    valid
        .then_some(())
        .ok_or("file signature does not match its extension")
}

pub(super) fn media_type(extension: &str) -> Option<&'static str> {
    match extension {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn canonical_extension(media_type: &str) -> &'static str {
    match media_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => unreachable!("validated image media type"),
    }
}
