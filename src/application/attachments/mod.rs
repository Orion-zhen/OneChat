use std::path::Path;

use crate::domain::AttachmentDraft;

mod audio;
mod image;
mod office;
mod pdf;
mod text;

pub use image::validate_image;

pub const MAX_ATTACHMENTS: usize = 10;
pub const MAX_AUDIO_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
pub(super) const MAX_TEXT_BYTES: u64 = 1024 * 1024;

pub fn load(
    path: &Path,
    vision: bool,
    audio_input: bool,
    parse_document_images: bool,
) -> Result<AttachmentDraft, String> {
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

    if let Some(result) = office::load(path, &name, &extension, size, parse_document_images) {
        result
    } else if audio::is_supported_extension(&extension) {
        audio::load(path, name, &extension, size, audio_input)
    } else if extension == "pdf" {
        pdf::load(path, name, size, vision)
    } else if image::media_type(&extension).is_some() {
        image::load(path, name, &extension, size, vision)
    } else {
        text::load(path, name, size)
    }
}
