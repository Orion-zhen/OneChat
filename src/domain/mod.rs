mod catalog;
mod conversation;
mod generation;
mod id;
mod preferences;
mod prompt;
mod reasoning;

pub use catalog::{GenerationConfig, Model, ModelCapabilities, Provider, ProviderKind};
pub use conversation::{
    AssistantResponse, Attachment, AttachmentDraft, AttachmentDraftFile, AttachmentFile,
    AttachmentFileKind, AttachmentKind, AutoTitleState, Conversation, MessageStatus, ToolRef,
    ToolSelection, Turn, UserMessage, active_turns, user_branches,
};
pub use generation::{
    GenerationError, GenerationErrorKind, GenerationEvent, GenerationRequest, Message,
    RequestError, RequestInfo, RequestStatus, TokenUsage, ToolDefinition, ToolExecution,
    ToolExecutionStatus, message_tool_calls,
};
pub use id::{Timestamp, new_id, now_timestamp};
pub use preferences::{
    AppSettings, DEFAULT_BACKGROUND_OPACITY, DEFAULT_CODE_FONT_FAMILY, DEFAULT_MESSAGE_FONT_SIZE,
    DEFAULT_MESSAGE_WIDTH_RATIO, DEFAULT_THEME_COLOR, DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT,
    DEFAULT_UI_FONT_FAMILY, MAX_BACKGROUND_OPACITY, MAX_MESSAGE_FONT_SIZE, MAX_MESSAGE_WIDTH_RATIO,
    MIN_BACKGROUND_OPACITY, MIN_MESSAGE_FONT_SIZE, MIN_MESSAGE_WIDTH_RATIO, SendMessageShortcut,
    Theme, normalize_font_families,
};
pub use prompt::{
    DEFAULT_PROMPT_COMMAND_TIMEOUT_MS, PromptEvaluation, PromptSnapshot, PromptVariableSource,
    SystemPromptPreset, prompt_variable_name_is_valid,
};
pub use reasoning::{
    CustomReasoningPreset, KnownReasoningFormat, KnownReasoningPreset, ModelReasoningConfig,
    PROVIDER_DEFAULT_REASONING_PRESET, ReasoningLevel, ReasoningParameter, ReasoningParameterValue,
    merge_json_patch,
};
