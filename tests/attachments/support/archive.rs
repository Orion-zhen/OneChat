use crate::attachments::*;

pub(crate) fn office_package(extension: &str, extras: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let (main_entry, content_type) = match extension {
        "docx" => (
            "word/document.xml",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
        ),
        "xlsx" => (
            "xl/workbook.xml",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
        ),
        "pptx" => (
            "ppt/presentation.xml",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
        ),
        _ => panic!("unsupported test Office extension: {extension}"),
    };
    let content_types = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/{main_entry}" ContentType="{content_type}"/></Types>"#
    );
    let relationships = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="{main_entry}"/></Relationships>"#
    );
    let mut entries = vec![
        ("[Content_Types].xml".into(), content_types.into_bytes()),
        ("_rels/.rels".into(), relationships.into_bytes()),
        (main_entry.into(), b"<root/>".to_vec()),
    ];
    entries.extend(extras);
    zip_entries(entries)
}

pub(crate) fn zip_entries(entries: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in entries {
        zip.start_file(name, options).unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

pub(crate) fn set_central_flags(bytes: &mut [u8], flags: u16) {
    for index in 0..bytes.len().saturating_sub(10) {
        if bytes[index..].starts_with(b"PK\x01\x02") {
            bytes[index + 8..index + 10].copy_from_slice(&flags.to_le_bytes());
        }
    }
}

pub(crate) fn set_central_uncompressed_size(bytes: &mut [u8], size: u32) {
    for index in 0..bytes.len().saturating_sub(28) {
        if bytes[index..].starts_with(b"PK\x01\x02") {
            bytes[index + 24..index + 28].copy_from_slice(&size.to_le_bytes());
        }
    }
}
