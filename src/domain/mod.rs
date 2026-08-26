mod catalog;
mod conversation;
mod generation;
mod id;
mod preferences;
mod prompt;
mod reasoning;

pub use catalog::{
    GenerationConfig, Model, ModelCapabilities, Provider, ProviderKind, format_compact_token_count,
};
pub use conversation::{
    AssistantBlock, AssistantResponse, Attachment, AttachmentDraft, AttachmentDraftFile,
    AttachmentFile, AttachmentFileKind, AttachmentKind, AudioAttachmentMetadata,
    AudioAttachmentSource, AutoTitleState, Conversation, MessageStatus, ToolRef, ToolSelection,
    Turn, UserMessage, active_turns, user_branches,
};
pub use generation::{
    GenerationError, GenerationErrorKind, GenerationEvent, GenerationRequest, Message,
    RequestContextInfo, RequestError, RequestInfo, RequestKind, RequestStatus, TokenUsage,
    ToolDefinition, ToolExecution, ToolExecutionStatus, continue_last_assistant,
    message_tool_calls,
};
pub use id::{Timestamp, new_id, now_timestamp};
pub use preferences::{
    AppSettings, DEFAULT_BACKGROUND_OPACITY, DEFAULT_CODE_FONT_FAMILY, DEFAULT_MESSAGE_FONT_SIZE,
    DEFAULT_MESSAGE_WIDTH_RATIO, DEFAULT_THEME_COLOR, DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT,
    DEFAULT_TRANSLATION_SYSTEM_PROMPT, DEFAULT_TRANSLATION_USER_PROMPT, DEFAULT_UI_FONT_FAMILY,
    HISTORY_LIMIT_SLIDER_MAX, HISTORY_LIMIT_SLIDER_MIN, HISTORY_LIMIT_SLIDER_STEP, HistoryLimit,
    MAX_BACKGROUND_OPACITY, MAX_LIMITED_HISTORY_TURNS, MAX_MESSAGE_FONT_SIZE,
    MAX_MESSAGE_WIDTH_RATIO, MIN_BACKGROUND_OPACITY, MIN_MESSAGE_FONT_SIZE,
    MIN_MESSAGE_WIDTH_RATIO, SendMessageShortcut, Theme, TitleModelSource, normalize_font_families,
};
pub use prompt::{
    BUILTIN_PROMPT_VARIABLES, DEFAULT_PROMPT_COMMAND_TIMEOUT_MS, PromptEvaluation, PromptPreset,
    PromptSnapshot, PromptVariableSource, prompt_variable_name_is_valid,
};
pub use reasoning::{
    CustomReasoningPreset, KnownReasoningFormat, KnownReasoningPreset, ModelReasoningConfig,
    PROVIDER_DEFAULT_REASONING_PRESET, ReasoningLevel, ReasoningParameter, ReasoningParameterValue,
    merge_json_patch,
};
