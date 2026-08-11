use undoc::{Block, Document, ErrorKind, FormatType, Paragraph, SectionMarkerStyle, Table, render};

use crate::domain::{
    AttachmentDraft, AttachmentDraftFile, AttachmentFileKind, AttachmentKind, new_id,
};

use super::super::MAX_TEXT_BYTES;
use super::media;

pub(super) fn load(
    bytes: Vec<u8>,
    name: String,
    expected_format: FormatType,
    parse_images: bool,
) -> Result<AttachmentDraft, String> {
    let mut document =
        undoc::parse_bytes(&bytes).map_err(|error| parser_error(&name, expected_format, error))?;
    if document.format != expected_format {
        return Err(format!(
            "Invalid {}: {name} contains {} content.",
            label(expected_format),
            label(document.format)
        ));
    }

    let images = if parse_images {
        media::extract(&mut document)
    } else {
        strip_images(&mut document);
        Vec::new()
    };
    let options = render_options(expected_format);
    let markdown = render::to_markdown(&document, &options).map_err(|error| {
        format!(
            "Could not render {} {name} as Markdown: {error}",
            label(expected_format)
        )
    })?;

    if markdown.len() as u64 > MAX_TEXT_BYTES {
        return Err(format!(
            "Extracted Markdown too large: {name} exceeds the 1 MiB extracted Markdown limit."
        ));
    }
    if markdown.trim().is_empty() {
        return Err(format!(
            "{name} is an empty {} document (no readable content).",
            label(expected_format)
        ));
    }

    let mut files = Vec::with_capacity(images.len() + 1);
    files.push(AttachmentDraftFile {
        name: "content.md".into(),
        kind: AttachmentFileKind::Text,
        media_type: "text/markdown".into(),
        bytes: markdown.into_bytes(),
    });
    files.extend(images);

    Ok(AttachmentDraft {
        id: new_id("attachment"),
        name,
        kind: AttachmentKind::Document,
        files,
        audio: None,
    })
}

fn strip_images(document: &mut Document) {
    for section in &mut document.sections {
        section.content.retain_mut(|block| match block {
            Block::Image { .. } => false,
            Block::Paragraph(paragraph) => {
                strip_paragraph_images(paragraph);
                true
            }
            Block::Table(table) => {
                strip_table_images(table);
                true
            }
            Block::PageBreak | Block::SectionBreak => true,
        });
        for paragraphs in [&mut section.header, &mut section.footer, &mut section.notes]
            .into_iter()
            .flatten()
        {
            for paragraph in paragraphs {
                strip_paragraph_images(paragraph);
            }
        }
    }
    document.resources.clear();
}

fn strip_table_images(table: &mut Table) {
    for cell in table.rows.iter_mut().flat_map(|row| &mut row.cells) {
        for paragraph in &mut cell.content {
            strip_paragraph_images(paragraph);
        }
        for nested_table in &mut cell.nested_tables {
            strip_table_images(nested_table);
        }
    }
}

fn strip_paragraph_images(paragraph: &mut Paragraph) {
    paragraph.images.clear();
}

fn render_options(format: FormatType) -> render::RenderOptions {
    let options = render::RenderOptions::default();
    match format {
        FormatType::Docx => options,
        FormatType::Xlsx | FormatType::Pptx => {
            options.with_section_markers(SectionMarkerStyle::Comment)
        }
    }
}

fn parser_error(name: &str, format: FormatType, error: undoc::Error) -> String {
    let format = label(format);
    if error.kind() == ErrorKind::Encrypted {
        format!("Encrypted {format}: {name} is an encrypted Office document.")
    } else {
        format!("Could not parse {format} {name}: {error}")
    }
}

fn label(format: FormatType) -> &'static str {
    match format {
        FormatType::Docx => "DOCX",
        FormatType::Xlsx => "XLSX",
        FormatType::Pptx => "PPTX",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_markers_are_only_enabled_for_sectioned_formats() {
        assert_eq!(
            render_options(FormatType::Docx).section_markers,
            SectionMarkerStyle::None
        );
        assert_eq!(
            render_options(FormatType::Xlsx).section_markers,
            SectionMarkerStyle::Comment
        );
        assert_eq!(
            render_options(FormatType::Pptx).section_markers,
            SectionMarkerStyle::Comment
        );
    }
}
