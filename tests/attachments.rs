use std::{
    fmt::Write as _,
    fs,
    io::{Cursor, Write},
    path::Path,
};

use onechat::{
    application::attachments::{load as load_attachment, validate_image},
    domain::{AttachmentDraft, AttachmentFileKind, AttachmentKind, Conversation, UserMessage},
    storage::Storage,
};
use tempfile::tempdir;

fn load(path: &Path, vision: bool) -> Result<AttachmentDraft, String> {
    load_attachment(path, vision, true)
}

#[path = "attachments/basic.rs"]
mod basic;
#[path = "attachments/docx.rs"]
mod docx;
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
