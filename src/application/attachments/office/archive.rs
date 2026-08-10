use std::{collections::HashSet, io::Cursor};

use undoc::FormatType;

const MAX_OFFICE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;
const MAX_ENTRY_BYTES: u64 = 32 * 1024 * 1024;
const MAX_UNCOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;
const CFB_SIGNATURE: &[u8] = b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1";
const COMMON_ENTRIES: [&str; 2] = ["[Content_Types].xml", "_rels/.rels"];

pub(super) fn read(
    path: &std::path::Path,
    name: &str,
    size: u64,
    format: FormatType,
) -> Result<Vec<u8>, String> {
    let label = label(format);
    if size > MAX_OFFICE_BYTES {
        return Err(format!(
            "Source file too large: {name} exceeds the 20 MiB {label} limit."
        ));
    }

    let bytes = std::fs::read(path).map_err(|error| format!("Could not read {name}: {error}"))?;
    if bytes.len() as u64 > MAX_OFFICE_BYTES {
        return Err(format!(
            "Source file too large: {name} exceeds the 20 MiB {label} limit."
        ));
    }
    if bytes.starts_with(CFB_SIGNATURE) {
        return Err(format!(
            "Encrypted {label}: {name} is an encrypted Office document."
        ));
    }

    inspect(&bytes, name, format)?;
    Ok(bytes)
}

fn inspect(bytes: &[u8], name: &str, format: FormatType) -> Result<(), String> {
    let label = label(format);
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| {
        format!("Invalid {label}: {name} is not a valid ZIP archive ({error}).")
    })?;
    if archive.len() > MAX_ENTRIES {
        return Err(format!(
            "Unsafe {label} archive: {name} contains too many ZIP entries (maximum {MAX_ENTRIES})."
        ));
    }

    let mut names = HashSet::with_capacity(archive.len());
    let mut total_size = 0_u64;
    for index in 0..archive.len() {
        let file = archive.by_index_raw(index).map_err(|error| {
            format!("Invalid {label}: {name} has a damaged ZIP entry ({error}).")
        })?;
        if file.encrypted() {
            return Err(format!(
                "Encrypted {label}: {name} is an encrypted Office document."
            ));
        }
        if file.size() > MAX_ENTRY_BYTES {
            return Err(format!(
                "Unsafe {label} archive: {name} contains a ZIP entry larger than 32 MiB: {}.",
                file.name()
            ));
        }
        total_size = total_size.checked_add(file.size()).ok_or_else(|| {
            format!(
                "Unsafe {label} archive: {name} declares an invalid total uncompressed ZIP size."
            )
        })?;
        if total_size > MAX_UNCOMPRESSED_BYTES {
            return Err(format!(
                "Unsafe {label} archive: {name} exceeds the 256 MiB uncompressed ZIP limit."
            ));
        }
        names.insert(file.name().to_string());
    }

    let required_entries = COMMON_ENTRIES.into_iter().chain([main_entry(format)]);
    if let Some(missing) = required_entries
        .into_iter()
        .find(|required| !names.contains(*required))
    {
        return Err(format!(
            "Invalid {label}: {name} is missing required OOXML entry {missing}."
        ));
    }
    Ok(())
}

fn main_entry(format: FormatType) -> &'static str {
    match format {
        FormatType::Docx => "word/document.xml",
        FormatType::Xlsx => "xl/workbook.xml",
        FormatType::Pptx => "ppt/presentation.xml",
    }
}

fn label(format: FormatType) -> &'static str {
    match format {
        FormatType::Docx => "DOCX",
        FormatType::Xlsx => "XLSX",
        FormatType::Pptx => "PPTX",
    }
}
