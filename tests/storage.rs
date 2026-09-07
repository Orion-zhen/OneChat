use std::fs;

use onechat::{
    application::generation::{ContextPolicy, GenerationStart, PreparedGeneration},
    domain::{
        AppSettings, Attachment, AttachmentDraft, AttachmentDraftFile, AttachmentFile,
        AttachmentFileKind, AttachmentKind, AudioAttachmentMetadata, AudioAttachmentSource,
        AutoTitleState, Conversation, HistoryLimit, MessageStatus, Model, PromptPreset,
        PromptVariableSource, Provider, ProviderKind, RequestStatus, TitleModelSource,
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
#[path = "storage/catalog_updates.rs"]
mod catalog_updates;
#[path = "storage/conversations.rs"]
mod conversations;
#[path = "storage/recovery.rs"]
mod recovery;
#[path = "storage/response.rs"]
mod response;
#[path = "storage/session.rs"]
mod session;
#[path = "storage/settings.rs"]
mod settings;
#[path = "storage/snapshot.rs"]
mod snapshot;
#[path = "storage/state.rs"]
mod state;
#[path = "storage/support.rs"]
mod support;
#[path = "storage/transactions.rs"]
mod transactions;

pub(crate) use support::*;
