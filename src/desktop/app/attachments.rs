use super::*;

use hayro::{RenderCache, RenderSettings, hayro_interpret::InterpreterSettings, hayro_syntax::Pdf};

const MAX_ATTACHMENTS: usize = 10;
const MAX_TEXT_BYTES: u64 = 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_PDF_BYTES: u64 = 20 * 1024 * 1024;
const MAX_PDF_PAGES: usize = 20;
const MAX_PDF_EDGE: f32 = 1600.0;

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

fn load_attachment(path: &std::path::Path, vision: bool) -> Result<AttachmentDraft, String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("Invalid attachment path: {}", path.display()))?
        .to_string();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let size = std::fs::metadata(path)
        .map_err(|error| format!("Could not read {name}: {error}"))?
        .len();

    if extension == "pdf" {
        if !vision {
            return Err(format!("{name} requires a model with vision support."));
        }
        if size > MAX_PDF_BYTES {
            return Err(format!("{name} exceeds the 20 MiB PDF limit."));
        }
        return load_pdf(path, name);
    }

    if let Some(media_type) = image_media_type(&extension) {
        if !vision {
            return Err(format!("{name} requires a model with vision support."));
        }
        if size > MAX_IMAGE_BYTES {
            return Err(format!("{name} exceeds the 10 MiB image limit."));
        }
        let bytes =
            std::fs::read(path).map_err(|error| format!("Could not read {name}: {error}"))?;
        validate_image(&bytes, media_type)
            .map_err(|error| format!("Invalid image {name}: {error}"))?;
        return Ok(AttachmentDraft {
            id: new_id("attachment"),
            name,
            kind: AttachmentKind::Image,
            files: vec![AttachmentDraftFile {
                extension: image_extension(media_type),
                media_type,
                bytes,
            }],
        });
    }

    if size > MAX_TEXT_BYTES {
        return Err(format!("{name} exceeds the 1 MiB text attachment limit."));
    }
    let bytes = std::fs::read(path).map_err(|error| format!("Could not read {name}: {error}"))?;
    std::str::from_utf8(&bytes).map_err(|_| format!("{name} is not a UTF-8 text file."))?;
    Ok(AttachmentDraft {
        id: new_id("attachment"),
        name,
        kind: AttachmentKind::Text,
        files: vec![AttachmentDraftFile {
            extension: "txt",
            media_type: "text/plain",
            bytes,
        }],
    })
}

fn load_pdf(path: &std::path::Path, name: String) -> Result<AttachmentDraft, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("Could not read {name}: {error}"))?;
    if !bytes.starts_with(b"%PDF-") {
        return Err(format!("Invalid PDF: {name}"));
    }
    let pdf = Pdf::new(bytes).map_err(|error| format!("Could not parse PDF {name}: {error:?}"))?;
    let pages = pdf.pages();
    if pages.is_empty() {
        return Err(format!("PDF contains no pages: {name}"));
    }
    if pages.len() > MAX_PDF_PAGES {
        return Err(format!(
            "{name} exceeds the {MAX_PDF_PAGES}-page PDF limit."
        ));
    }

    let cache = RenderCache::new();
    let interpreter = InterpreterSettings::default();
    let files = pages
        .iter()
        .map(|page| {
            let (width, height) = page.render_dimensions();
            let scale = (MAX_PDF_EDGE / width.max(height)).min(2.0);
            let pixmap = hayro::render(
                page,
                &cache,
                &interpreter,
                &RenderSettings {
                    x_scale: scale,
                    y_scale: scale,
                    bg_color: hayro::vello_cpu::color::palette::css::WHITE,
                    ..Default::default()
                },
            );
            let bytes = pixmap
                .into_png()
                .map_err(|error| format!("Could not render {name}: {error}"))?;
            Ok(AttachmentDraftFile {
                extension: "png",
                media_type: "image/png",
                bytes,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(AttachmentDraft {
        id: new_id("attachment"),
        name,
        kind: AttachmentKind::Pdf,
        files,
    })
}

fn image_media_type(extension: &str) -> Option<&'static str> {
    match extension {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn image_extension(media_type: &str) -> &'static str {
    match media_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => unreachable!(),
    }
}

fn validate_image(bytes: &[u8], media_type: &str) -> Result<(), &'static str> {
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
