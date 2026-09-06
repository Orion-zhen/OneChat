use std::path::Path;

use crate::domain::{
    AttachmentDraft, AttachmentDraftFile, AttachmentFileKind, AttachmentKind, new_id,
};

pub(super) fn load(path: &Path, name: String) -> Result<AttachmentDraft, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("Could not read {name}: {error}"))?;
    std::str::from_utf8(&bytes).map_err(|_| format!("{name} is not a UTF-8 text file."))?;

    Ok(AttachmentDraft {
        id: new_id("attachment"),
        name,
        kind: AttachmentKind::Text,
        files: vec![AttachmentDraftFile {
            name: "content.txt".into(),
            kind: AttachmentFileKind::Text,
            media_type: "text/plain".into(),
            bytes,
        }],
        audio: None,
    })
}
