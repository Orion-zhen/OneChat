use std::{
    fmt::Write as _,
    fs,
    io::{Cursor, Write},
    path::Path,
};

use onechat::{
    application::attachments::{
        LoadManyOptions, MAX_AUDIO_BYTES, load as load_attachment, load_many, validate_image,
    },
    domain::{
        AttachmentDraft, AttachmentFileKind, AttachmentKind, AudioAttachmentSource, Conversation,
        UserMessage,
    },
    storage::Storage,
};
use tempfile::tempdir;

fn load(path: &Path, vision: bool) -> Result<AttachmentDraft, String> {
    load_attachment(path, vision, false, true)
}

fn load_audio(path: &Path, audio_input: bool) -> Result<AttachmentDraft, String> {
    load_attachment(path, false, audio_input, true)
}

#[path = "attachments/audio.rs"]
mod audio;
#[path = "attachments/basic.rs"]
mod basic;
#[path = "attachments/docx.rs"]
mod docx;
#[path = "attachments/many.rs"]
mod many;
#[path = "attachments/office.rs"]
mod office;
#[path = "attachments/pptx.rs"]
mod pptx;
#[path = "attachments/support/mod.rs"]
mod support;
#[path = "attachments/validation.rs"]
mod validation;
#[path = "attachments/xlsx.rs"]
mod xlsx;

pub(crate) use support::*;
