use std::io::Cursor;

use image::ImageFormat;
use undoc::Document;

use crate::domain::{AttachmentDraftFile, AttachmentFileKind};

use super::super::{MAX_IMAGE_BYTES, validate_image};

const MAX_IMAGES: usize = 20;
const MAX_IMAGE_TOTAL_BYTES: u64 = 50 * 1024 * 1024;

pub(super) fn extract(document: &mut Document) -> Vec<AttachmentDraftFile> {
    let mut resource_ids = document.resources.keys().cloned().collect::<Vec<_>>();
    resource_ids.sort();

    let mut files = Vec::new();
    let mut total_bytes = 0_u64;
    for (index, resource_id) in resource_ids.into_iter().enumerate() {
        let resource = document
            .resources
            .get_mut(&resource_id)
            .expect("resource ID came from the document");
        let format = MediaFormat::from_resource(resource);
        let name = format!("image-{:03}.{}", index + 1, format.extension());
        resource.filename = Some(name.clone());

        if files.len() >= MAX_IMAGES || resource.data.len() as u64 > MAX_IMAGE_BYTES {
            continue;
        }
        let Some((media_type, bytes)) = format.prepare(&resource.data) else {
            continue;
        };
        let size = bytes.len() as u64;
        if size > MAX_IMAGE_BYTES
            || total_bytes
                .checked_add(size)
                .is_none_or(|total| total > MAX_IMAGE_TOTAL_BYTES)
        {
            continue;
        }

        total_bytes += size;
        files.push(AttachmentDraftFile {
            name,
            kind: AttachmentFileKind::Image,
            media_type: media_type.into(),
            bytes,
        });
    }
    files
}

#[derive(Clone, Copy)]
enum MediaFormat {
    Png,
    Jpeg,
    Gif,
    Webp,
    Bmp,
    Tiff,
    Svg,
    Wmf,
    Emf,
    Unknown,
}

impl MediaFormat {
    fn from_resource(resource: &undoc::model::Resource) -> Self {
        let extension = resource
            .filename
            .as_deref()
            .and_then(|name| name.rsplit_once('.').map(|(_, extension)| extension))
            .unwrap_or_default()
            .to_ascii_lowercase();
        match extension.as_str() {
            "png" => Self::Png,
            "jpg" | "jpeg" => Self::Jpeg,
            "gif" => Self::Gif,
            "webp" => Self::Webp,
            "bmp" => Self::Bmp,
            "tif" | "tiff" => Self::Tiff,
            "svg" => Self::Svg,
            "wmf" => Self::Wmf,
            "emf" => Self::Emf,
            _ => match resource.mime_type.as_deref() {
                Some("image/png") => Self::Png,
                Some("image/jpeg") => Self::Jpeg,
                Some("image/gif") => Self::Gif,
                Some("image/webp") => Self::Webp,
                Some("image/bmp") => Self::Bmp,
                Some("image/tiff") => Self::Tiff,
                Some("image/svg+xml") => Self::Svg,
                Some("image/x-wmf") => Self::Wmf,
                Some("image/x-emf") => Self::Emf,
                _ => Self::Unknown,
            },
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Png | Self::Bmp | Self::Tiff => "png",
            Self::Jpeg => "jpg",
            Self::Gif => "gif",
            Self::Webp => "webp",
            Self::Svg => "svg",
            Self::Wmf => "wmf",
            Self::Emf => "emf",
            Self::Unknown => "bin",
        }
    }

    fn prepare(self, bytes: &[u8]) -> Option<(&'static str, Vec<u8>)> {
        let media_type = match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
            Self::Bmp | Self::Tiff => return convert_to_png(bytes, self),
            Self::Svg | Self::Wmf | Self::Emf | Self::Unknown => return None,
        };
        validate_image(bytes, media_type).ok()?;
        Some((media_type, bytes.to_vec()))
    }
}

fn convert_to_png(bytes: &[u8], format: MediaFormat) -> Option<(&'static str, Vec<u8>)> {
    let format = match format {
        MediaFormat::Bmp => ImageFormat::Bmp,
        MediaFormat::Tiff => ImageFormat::Tiff,
        _ => unreachable!("only BMP and TIFF are converted"),
    };
    let image = image::load_from_memory_with_format(bytes, format).ok()?;
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, ImageFormat::Png).ok()?;
    Some(("image/png", output.into_inner()))
}
