use std::{collections::BTreeMap, sync::Arc, time::Duration};

use onechat::{
    application::generation::{
        ContextPolicy, GenerationManager, GenerationStart, GenerationUpdate, PreparedGeneration,
        apply_event, history_for_new_turn, history_for_turn, history_preview_for_new_turn,
        run_generation,
    },
    domain::{
        AssistantResponse, Attachment, AttachmentDraft, AttachmentDraftFile, AttachmentFileKind,
        AttachmentKind, Conversation, CustomReasoningPreset, GenerationConfig, GenerationError,
        GenerationErrorKind, GenerationEvent, HistoryLimit, KnownReasoningFormat, Message,
        MessageStatus, Model, ModelReasoningConfig, PromptVariableSource, Provider, ProviderKind,
        ReasoningParameter, ReasoningParameterValue, RequestInfo, RequestStatus, TokenUsage, Turn,
        UserMessage, merge_json_patch,
    },
    mcp::McpManager,
    storage::Storage,
};
use serde_json::{Map, Value, json};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

#[path = "generation/attachments.rs"]
mod attachments;
#[path = "generation/branching.rs"]
mod branching;
#[path = "generation/context_window.rs"]
mod context_window;
#[path = "generation/history.rs"]
mod history;
#[path = "generation/manager.rs"]
mod manager;
#[path = "generation/preparation.rs"]
mod preparation;
#[path = "generation/reasoning.rs"]
mod reasoning;
#[path = "generation/streaming.rs"]
mod streaming;
#[path = "generation/support.rs"]
mod support;

pub(crate) use support::*;
