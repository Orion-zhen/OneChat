use crate::domain::{AttachmentFileKind, AttachmentKind};

pub(super) fn attachment_detail(
    name: &str,
    kind: AttachmentKind,
    files: impl IntoIterator<Item = AttachmentFileKind>,
) -> String {
    let (file_count, image_count) = files.into_iter().fold((0, 0), |(files, images), kind| {
        (
            files + 1,
            images + usize::from(kind == AttachmentFileKind::Image),
        )
    });
    match kind {
        AttachmentKind::Text => "Text document".into(),
        AttachmentKind::Image => "Image".into(),
        AttachmentKind::Pdf => format!(
            "PDF · {file_count} page{}",
            if file_count == 1 { "" } else { "s" }
        ),
        AttachmentKind::Document => {
            let extension = std::path::Path::new(name).extension();
            let document = match extension {
                Some(extension) if extension.eq_ignore_ascii_case("docx") => "Word document",
                Some(extension) if extension.eq_ignore_ascii_case("xlsx") => "Excel spreadsheet",
                Some(extension) if extension.eq_ignore_ascii_case("pptx") => "PowerPoint slides",
                _ => "Document",
            };
            if image_count == 0 {
                document.into()
            } else {
                format!(
                    "{document} · {image_count} image{}",
                    if image_count == 1 { "" } else { "s" }
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn office_details_are_format_specific_and_only_count_images() {
        let cases = [
            ("Report.DOCX", "Word document"),
            ("Budget.XLSX", "Excel spreadsheet"),
            ("Deck.PPTX", "PowerPoint slides"),
        ];

        for (name, detail) in cases {
            assert_eq!(
                attachment_detail(name, AttachmentKind::Document, [AttachmentFileKind::Text]),
                detail
            );
            assert_eq!(
                attachment_detail(
                    name,
                    AttachmentKind::Document,
                    [
                        AttachmentFileKind::Text,
                        AttachmentFileKind::Image,
                        AttachmentFileKind::Image,
                    ],
                ),
                format!("{detail} · 2 images")
            );
        }
    }

    #[test]
    fn standard_attachment_details_are_consistent() {
        assert_eq!(
            attachment_detail(
                "document.pdf",
                AttachmentKind::Pdf,
                [AttachmentFileKind::Image],
            ),
            "PDF · 1 page"
        );
        assert_eq!(
            attachment_detail("photo.png", AttachmentKind::Image, []),
            "Image"
        );
        assert_eq!(
            attachment_detail("notes.txt", AttachmentKind::Text, []),
            "Text document"
        );
    }
}
