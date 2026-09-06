use std::path::Path;

use hayro::{RenderCache, RenderSettings, hayro_interpret::InterpreterSettings, hayro_syntax::Pdf};

use crate::domain::{
    AttachmentDraft, AttachmentDraftFile, AttachmentFileKind, AttachmentKind, new_id,
};

const MAX_PDF_EDGE: f32 = 1600.0;

pub(super) fn load(path: &Path, name: String, vision: bool) -> Result<AttachmentDraft, String> {
    if !vision {
        return Err(format!("{name} requires a model with vision support."));
    }

    let bytes = std::fs::read(path).map_err(|error| format!("Could not read {name}: {error}"))?;
    if !bytes.starts_with(b"%PDF-") {
        return Err(format!("Invalid PDF: {name}"));
    }
    let pdf = Pdf::new(bytes).map_err(|error| format!("Could not parse PDF {name}: {error:?}"))?;
    let pages = pdf.pages();
    if pages.is_empty() {
        return Err(format!("PDF contains no pages: {name}"));
    }

    let cache = RenderCache::new();
    let interpreter = InterpreterSettings::default();
    let files = pages
        .iter()
        .enumerate()
        .map(|(index, page)| {
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
                name: format!("page-{:03}.png", index + 1),
                kind: AttachmentFileKind::Image,
                media_type: "image/png".into(),
                bytes,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(AttachmentDraft {
        id: new_id("attachment"),
        name,
        kind: AttachmentKind::Pdf,
        files,
        audio: None,
    })
}
