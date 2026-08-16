use std::path::{Path, PathBuf};

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
pub(super) const MAX_TEXT_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct LoadManyOptions {
    pub remaining: usize,
    pub vision: bool,
    pub audio_input: bool,
    pub parse_document_images: bool,
}

pub fn load_many(
    paths: Vec<PathBuf>,
    options: LoadManyOptions,
) -> Result<Vec<AttachmentDraft>, String> {
    if paths.len() > options.remaining {
        return Err(format!(
            "Select at most {} more attachment{}.",
            options.remaining,
            if options.remaining == 1 { "" } else { "s" }
        ));
    }
    paths
        .into_iter()
        .map(|path| {
            if path.is_dir() {
                Err(format!(
                    "Folders cannot be added as attachments: {}",
                    path.display()
                ))
            } else {
                load(
                    &path,
                    options.vision,
                    options.audio_input,
                    options.parse_document_images,
                )
            }
        })
        .collect()
}

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
