use std::fs;

use onechat::{
    application::generation::{ContextPolicy, GenerationStart, PreparedGeneration},
    domain::{
        AppSettings, AttachmentDraft, AttachmentDraftFile, AttachmentFileKind, AttachmentKind,
        AutoTitleState, Conversation, HistoryLimit, MessageStatus, Model, PromptVariableSource,
        Provider, ProviderKind, RequestContextInfo, RequestInfo, RequestStatus, SystemPromptPreset,
        ToolExecution, ToolExecutionStatus, Turn, UserMessage, active_turns,
    },
    storage::{Storage, WindowMode, WindowState},
};
use tempfile::{TempDir, tempdir};

#[path = "storage/attachment_messages.rs"]
mod attachment_messages;
#[path = "storage/attachment_storage.rs"]
mod attachment_storage;
#[path = "storage/catalog.rs"]
mod catalog;
#[path = "storage/conversations.rs"]
mod conversations;
#[path = "storage/recovery.rs"]
mod recovery;
#[path = "storage/settings.rs"]
mod settings;
#[path = "storage/support.rs"]
mod support;

pub(crate) use support::*;
