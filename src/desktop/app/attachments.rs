use std::{path::PathBuf, sync::Arc};

use gpui::{Context, prelude::*};

use super::{MessageEditorTarget, OneChat};
use crate::{
    application::attachments::{
        MAX_ATTACHMENTS, MAX_IMAGE_BYTES, load as load_attachment, validate_image,
    },
    domain::{
        AttachmentDraft, AttachmentDraftFile, AttachmentFileKind, AttachmentKind,
        ModelCapabilities, new_id,
    },
};

struct ComposerAttachmentLoad {
    conversation_id: String,
    vision: bool,
    audio_input: bool,
    parse_document_images: bool,
    remaining: usize,
    revision: u64,
}

mod composer;
mod message_editor;

impl OneChat {
    pub(crate) fn attachment_file_path(
        &self,
        file: &crate::domain::AttachmentFile,
    ) -> Option<std::path::PathBuf> {
        let conversation_id = &self.current_conversation()?.id;
        self.services
            .storage
            .attachment_path(conversation_id, &file.path)
            .ok()
    }
}

fn attachment_capability_error(
    capabilities: &ModelCapabilities,
    attachments: &[AttachmentDraft],
) -> Option<&'static str> {
    let vision = !capabilities.vision
        && attachments
            .iter()
            .any(|attachment| attachment.kind.requires_vision());
    let audio = !capabilities.audio_input
        && attachments
            .iter()
            .any(|attachment| attachment.kind.requires_audio_input());
    match (vision, audio) {
        (true, true) => {
            Some("The selected model does not accept image, PDF, or audio attachments.")
        }
        (true, false) => Some("The selected model does not accept image or PDF attachments."),
        (false, true) => Some("The selected model does not accept audio attachments."),
        (false, false) => None,
    }
}

fn attachment_preview(attachment: &AttachmentDraft) -> Option<Arc<gpui::Image>> {
    if !attachment.kind.requires_vision() {
        return None;
    }
    let file = attachment
        .files
        .iter()
        .find(|file| file.kind == AttachmentFileKind::Image)?;
    let format = gpui::ImageFormat::from_mime_type(&file.media_type)?;
    Some(Arc::new(gpui::Image::from_bytes(
        format,
        file.bytes.clone(),
    )))
}

fn clipboard_image_attachment(
    image: gpui::Image,
    number: usize,
) -> Result<AttachmentDraft, String> {
    if image.bytes().len() as u64 > MAX_IMAGE_BYTES {
        return Err("The pasted image exceeds the 10 MiB image limit.".into());
    }
    let (extension, media_type, bytes) = match image.format() {
        gpui::ImageFormat::Jpeg => ("jpg", "image/jpeg", image.bytes().to_vec()),
        gpui::ImageFormat::Png => ("png", "image/png", image.bytes().to_vec()),
        gpui::ImageFormat::Gif => ("gif", "image/gif", image.bytes().to_vec()),
        gpui::ImageFormat::Webp => ("webp", "image/webp", image.bytes().to_vec()),
        gpui::ImageFormat::Svg => {
            return Err("Pasted SVG images are not supported.".into());
        }
        format => {
            let format = match format {
                gpui::ImageFormat::Bmp => image::ImageFormat::Bmp,
                gpui::ImageFormat::Tiff => image::ImageFormat::Tiff,
                gpui::ImageFormat::Ico => image::ImageFormat::Ico,
                gpui::ImageFormat::Pnm => image::ImageFormat::Pnm,
                _ => unreachable!(),
            };
            let decoded = image::load_from_memory_with_format(image.bytes(), format)
                .map_err(|error| format!("Could not decode pasted image: {error}"))?;
            let mut bytes = std::io::Cursor::new(Vec::new());
            decoded
                .write_to(&mut bytes, image::ImageFormat::Png)
                .map_err(|error| format!("Could not convert pasted image: {error}"))?;
            ("png", "image/png", bytes.into_inner())
        }
    };
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err("The converted pasted image exceeds the 10 MiB image limit.".into());
    }
    validate_image(&bytes, media_type).map_err(|error| format!("Invalid pasted image: {error}"))?;
    Ok(AttachmentDraft {
        id: new_id("attachment"),
        name: format!("Pasted image {number}.{extension}"),
        kind: AttachmentKind::Image,
        files: vec![AttachmentDraftFile {
            name: format!("content.{extension}"),
            kind: AttachmentFileKind::Image,
            media_type: media_type.into(),
            bytes,
        }],
        audio: None,
    })
}
