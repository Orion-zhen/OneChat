use std::path::{Path, PathBuf};

use crate::domain::AttachmentDraft;

mod audio;
mod image;
mod office;
mod pdf;
mod text;

pub use image::validate_image;

#[derive(Clone, Copy, Debug)]
pub struct LoadManyOptions {
    pub vision: bool,
    pub audio_input: bool,
    pub parse_document_images: bool,
}

pub fn load_many(
    paths: Vec<PathBuf>,
    options: LoadManyOptions,
) -> Result<Vec<AttachmentDraft>, String> {
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
    if let Some(result) = office::load(path, &name, &extension, parse_document_images) {
        result
    } else if audio::is_supported_extension(&extension) {
        audio::load(path, name, &extension, audio_input)
    } else if extension == "pdf" {
        pdf::load(path, name, vision)
    } else if image::media_type(&extension).is_some() {
        image::load(path, name, &extension, vision)
    } else {
        text::load(path, name)
    }
}
