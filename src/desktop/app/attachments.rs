use std::{fmt::Display, path::PathBuf, sync::Arc};

use gpui::{Context, prelude::*};

use super::{MessageEditorTarget, OneChat};
use crate::{
    application::attachments::{
        LoadManyOptions, MAX_ATTACHMENTS, MAX_IMAGE_BYTES, load_many, validate_image,
    },
    domain::{
        Attachment, AttachmentDraft, AttachmentDraftFile, AttachmentFile, AttachmentFileKind,
        AttachmentKind, ModelCapabilities, new_id,
    },
};

struct ComposerAttachmentLoad {
    conversation_id: String,
    options: LoadManyOptions,
    revision: u64,
}

enum AttachmentPathSelection {
    Selected(Vec<PathBuf>),
    Cancelled,
    Error(String),
}

fn attachment_path_prompt_options() -> gpui::PathPromptOptions {
    gpui::PathPromptOptions {
        files: true,
        directories: false,
        multiple: true,
        prompt: Some("Select Attachments".into()),
    }
}

fn normalize_attachment_path_selection<PromptError: Display, ChannelError: Display>(
    result: Result<Result<Option<Vec<PathBuf>>, PromptError>, ChannelError>,
) -> AttachmentPathSelection {
    match result {
        Ok(Ok(Some(paths))) => AttachmentPathSelection::Selected(paths),
        Ok(Ok(None)) => AttachmentPathSelection::Cancelled,
        Ok(Err(error)) => {
            AttachmentPathSelection::Error(format!("Could not open attachments: {error}"))
        }
        Err(error) => AttachmentPathSelection::Error(format!(
            "Attachment picker closed unexpectedly: {error}"
        )),
    }
}

mod composer;
mod message_editor;

impl OneChat {
    pub(crate) fn attachment_file_path(
        &self,
        file: &crate::domain::AttachmentFile,
    ) -> Option<std::path::PathBuf> {
        let conversation = self.current_conversation()?;
        (!conversation.temporary)
            .then(|| {
                self.services
                    .storage
                    .attachment_path(&conversation.id, &file.path)
                    .ok()
            })
            .flatten()
    }

    pub(crate) fn temporary_attachment_bytes(
        &self,
        file: &crate::domain::AttachmentFile,
    ) -> Option<&[u8]> {
        self.chat
            .temporary_attachment_files
            .get(&file.path)
            .map(Vec::as_slice)
    }

    pub(crate) fn temporary_attachment_image(
        &self,
        file: &crate::domain::AttachmentFile,
    ) -> Option<Arc<gpui::Image>> {
        let format = gpui::ImageFormat::from_mime_type(&file.media_type)?;
        Some(Arc::new(gpui::Image::from_bytes(
            format,
            self.temporary_attachment_bytes(file)?.to_vec(),
        )))
    }

    pub(crate) fn store_temporary_attachments(
        &mut self,
        drafts: &[AttachmentDraft],
    ) -> Result<Vec<Attachment>, String> {
        let mut files = Vec::new();
        let attachments = drafts
            .iter()
            .map(|draft| {
                draft
                    .validate_files()
                    .map_err(|error| format!("invalid attachment {}: {error}", draft.name))?;
                let stored_files = draft
                    .files
                    .iter()
                    .map(|file| {
                        let path = format!("attachments/{}/{}", draft.id, file.name);
                        files.push((path.clone(), file.bytes.clone()));
                        AttachmentFile {
                            name: file.name.clone(),
                            kind: file.kind,
                            path,
                            media_type: file.media_type.clone(),
                        }
                    })
                    .collect();
                Ok(Attachment {
                    id: draft.id.clone(),
                    name: draft.name.clone(),
                    kind: draft.kind,
                    files: stored_files,
                    audio: draft.audio.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        self.chat.temporary_attachment_files.extend(files);
        Ok(attachments)
    }

    pub(crate) fn remove_temporary_attachments(&mut self, attachments: &[Attachment]) {
        for file in attachments.iter().flat_map(|attachment| &attachment.files) {
            self.chat.temporary_attachment_files.remove(&file.path);
        }
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
