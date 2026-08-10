use std::path::Path;

use undoc::FormatType;

use crate::domain::AttachmentDraft;

mod archive;
mod document;
mod media;

pub(super) fn load(
    path: &Path,
    name: &str,
    extension: &str,
    size: u64,
    parse_images: bool,
) -> Option<Result<AttachmentDraft, String>> {
    let format = match extension {
        "docx" => FormatType::Docx,
        "xlsx" => FormatType::Xlsx,
        "pptx" => FormatType::Pptx,
        "doc" | "docm" => return Some(unsupported(name, extension, "docx")),
        "xls" | "xlsm" => return Some(unsupported(name, extension, "xlsx")),
        "ppt" | "pptm" => return Some(unsupported(name, extension, "pptx")),
        _ => return None,
    };

    Some(
        archive::read(path, name, size, format)
            .and_then(|bytes| document::load(bytes, name.to_string(), format, parse_images)),
    )
}

fn unsupported(name: &str, extension: &str, target: &str) -> Result<AttachmentDraft, String> {
    Err(format!(
        "Unsupported Office format: {name} uses .{extension}; convert it to .{target}."
    ))
}
