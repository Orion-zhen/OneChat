use std::path::Path;

use hayro::{RenderCache, RenderSettings, hayro_interpret::InterpreterSettings, hayro_syntax::Pdf};

use crate::domain::{AttachmentDraft, AttachmentDraftFile, AttachmentKind, new_id};

pub const MAX_ATTACHMENTS: usize = 10;
pub const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

const MAX_TEXT_BYTES: u64 = 1024 * 1024;
const MAX_PDF_BYTES: u64 = 20 * 1024 * 1024;
const MAX_PDF_PAGES: usize = 20;
const MAX_PDF_EDGE: f32 = 1600.0;

pub fn load(path: &Path, vision: bool) -> Result<AttachmentDraft, String> {
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

fn load_pdf(path: &Path, name: String) -> Result<AttachmentDraft, String> {
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
        _ => unreachable!("validated image media type"),
    }
}
