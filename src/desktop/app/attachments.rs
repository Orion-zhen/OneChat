use std::sync::Arc;

use gpui::{Context, prelude::*};

use super::OneChat;
use crate::{
    application::attachments::{
        MAX_ATTACHMENTS, MAX_IMAGE_BYTES, load as load_attachment, validate_image,
    },
    domain::{AttachmentDraft, AttachmentDraftFile, AttachmentKind, new_id},
};

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

    pub(crate) fn add_attachments(&mut self, cx: &mut Context<Self>) {
        if self.is_current_generating() || self.chat.attachments_loading {
            return;
        }
        let Some(model) = self.current_model() else {
            self.data.error = Some("Choose a model before adding attachments.".into());
            cx.notify();
            return;
        };
        let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone())
        else {
            self.data.error = Some("Create or select a conversation first.".into());
            cx.notify();
            return;
        };
        if self.chat.attachments.len() >= MAX_ATTACHMENTS {
            self.data.error = Some(format!(
                "A message can contain at most {MAX_ATTACHMENTS} attachments."
            ));
            cx.notify();
            return;
        }

        let vision = model.capabilities.vision;
        let remaining = MAX_ATTACHMENTS - self.chat.attachments.len();
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Select Attachments".into()),
        });
        self.chat.attachments_loading = true;
        self.chat.attachments_revision = self.chat.attachments_revision.wrapping_add(1);
        let revision = self.chat.attachments_revision;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let selected = match paths.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Ok(None)) => {
                    let _ = this.update(cx, |this, cx| {
                        if this.chat.attachments_revision != revision {
                            return;
                        }
                        this.chat.attachments_loading = false;
                        cx.notify();
                    });
                    return;
                }
                Ok(Err(error)) => {
                    let _ = this.update(cx, |this, cx| {
                        if this.chat.attachments_revision != revision {
                            return;
                        }
                        this.chat.attachments_loading = false;
                        this.data.error = Some(format!("Could not open attachments: {error}"));
                        cx.notify();
                    });
                    return;
                }
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        if this.chat.attachments_revision != revision {
                            return;
                        }
                        this.chat.attachments_loading = false;
                        this.data.error =
                            Some(format!("Attachment picker closed unexpectedly: {error}"));
                        cx.notify();
                    });
                    return;
                }
            };

            let result = cx
                .background_spawn(async move {
                    if selected.len() > remaining {
                        return Err(format!(
                            "Select at most {remaining} more attachment{}.",
                            if remaining == 1 { "" } else { "s" }
                        ));
                    }
                    selected
                        .into_iter()
                        .map(|path| load_attachment(&path, vision))
                        .collect::<Result<Vec<_>, _>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.chat.attachments_revision != revision
                    || this.current_conversation().map(|value| &value.id) != Some(&conversation_id)
                {
                    return;
                }
                this.chat.attachments_loading = false;
                match result {
                    Ok(attachments)
                        if this.current_model().is_some_and(|model| {
                            !model.capabilities.vision
                                && attachments
                                    .iter()
                                    .any(|attachment| attachment.kind != AttachmentKind::Text)
                        }) =>
                    {
                        this.data.error =
                            Some("The selected model only accepts text attachments.".into());
                    }
                    Ok(attachments) => {
                        for attachment in &attachments {
                            if let Some(preview) = attachment_preview(attachment) {
                                this.chat
                                    .attachment_previews
                                    .insert(attachment.id.clone(), preview);
                            }
                        }
                        this.chat.attachments.extend(attachments);
                    }
                    Err(error) => this.data.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn paste_composer_image(&mut self, cx: &mut Context<Self>) {
        let Some(image) = cx
            .read_from_clipboard()
            .and_then(|clipboard| clipboard.entries.into_iter().next())
            .and_then(|entry| match entry {
                gpui::ClipboardEntry::Image(image) => Some(image),
                _ => None,
            })
        else {
            return;
        };
        if !self
            .current_model()
            .is_some_and(|model| model.capabilities.vision)
        {
            return;
        }
        cx.stop_propagation();
        if self.is_current_generating() || self.chat.attachments_loading {
            return;
        }
        let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone())
        else {
            return;
        };
        if self.chat.attachments.len() >= MAX_ATTACHMENTS {
            self.data.error = Some(format!(
                "A message can contain at most {MAX_ATTACHMENTS} attachments."
            ));
            cx.notify();
            return;
        }

        let number = self.chat.attachments.len() + 1;
        self.chat.attachments_loading = true;
        self.chat.attachments_revision = self.chat.attachments_revision.wrapping_add(1);
        let revision = self.chat.attachments_revision;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { clipboard_image_attachment(image, number) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.chat.attachments_revision != revision
                    || this.current_conversation().map(|value| &value.id) != Some(&conversation_id)
                {
                    return;
                }
                this.chat.attachments_loading = false;
                if !this
                    .current_model()
                    .is_some_and(|model| model.capabilities.vision)
                {
                    this.data.error =
                        Some("The selected model does not accept pasted images.".into());
                } else {
                    match result {
                        Ok(attachment) => {
                            if let Some(preview) = attachment_preview(&attachment) {
                                this.chat
                                    .attachment_previews
                                    .insert(attachment.id.clone(), preview);
                            }
                            this.chat.attachments.push(attachment);
                        }
                        Err(error) => this.data.error = Some(error),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn remove_attachment(&mut self, id: String, cx: &mut Context<Self>) {
        self.chat
            .attachments
            .retain(|attachment| attachment.id != id);
        self.chat.attachment_previews.remove(&id);
        cx.notify();
    }
}

fn attachment_preview(attachment: &AttachmentDraft) -> Option<Arc<gpui::Image>> {
    let file = attachment.files.first()?;
    let format = gpui::ImageFormat::from_mime_type(file.media_type)?;
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
            extension,
            media_type,
            bytes,
        }],
    })
}
