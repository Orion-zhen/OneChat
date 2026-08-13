mod attachments;
mod bootstrap;
mod chat;
mod conversation_peek;
mod conversations;
mod data;
mod export;
mod generation;
mod messages;
mod motion;
mod navigation;
mod overlays;
mod playback;
mod recording;
mod settings;
mod state;
mod tokio_bridge;
mod tts;

use motion::*;
pub use navigation::{ConversationGroup, Page};
pub(crate) use playback::{attachment_source_id, tts_combined_source_id, tts_segment_source_id};
use state::*;
pub(crate) use state::{ConversationPeekContent, GenerationBorderClock, PickerOverlay};
pub(crate) use tts::{TtsOperationKind, TtsState};

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{Context, Entity, FocusHandle, Render, Window, prelude::*};
use gpui_component::input::{InputState, TextareaState};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    desktop::ui::shell,
    domain::{Attachment, AttachmentDraft},
    markdown::MarkdownDocument,
};

#[derive(Clone, Debug)]
pub(crate) enum ConnectionTestStatus {
    Testing,
    Connected,
    Failed(String),
}

#[derive(Clone, Debug)]
pub(crate) enum DestructiveAction {
    DeleteConversation { id: String },
    DeleteProvider { id: String },
    DeleteModel { id: String },
    DeletePromptPreset { name: String },
    DeletePromptVariable { name: String },
    DeleteMcpServer { id: String },
    ClearContext { conversation_id: String },
}

impl DestructiveAction {
    fn title(&self) -> &'static str {
        match self {
            Self::DeleteConversation { .. } => "Delete Conversation?",
            Self::DeleteProvider { .. } => "Delete Provider?",
            Self::DeleteModel { .. } => "Delete Model?",
            Self::DeletePromptPreset { .. } => "Delete Prompt Preset?",
            Self::DeletePromptVariable { .. } => "Delete Prompt Variable?",
            Self::DeleteMcpServer { .. } => "Delete MCP Server?",
            Self::ClearContext { .. } => "Clear Conversation?",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::ClearContext { .. } => {
                "This removes the conversation context used for future responses."
            }
            Self::DeletePromptVariable { .. } => {
                "Prompts that reference this variable will stop working. This cannot be undone."
            }
            _ => "This action cannot be undone.",
        }
    }
}

struct RenameEditor {
    conversation_id: String,
    input: Entity<InputState>,
}

enum MessageEditorTarget {
    User(String),
    Assistant(String),
}

pub(crate) struct AssistantOutputEditor {
    pub(crate) block_id: String,
    pub(crate) input: Entity<TextareaState>,
}

pub(crate) struct MessageEditor {
    target: MessageEditorTarget,
    pub(crate) input: Entity<TextareaState>,
    pub(crate) output_editors: Vec<AssistantOutputEditor>,
    pub(crate) attachments: Vec<Attachment>,
    pub(crate) attachment_drafts: Vec<AttachmentDraft>,
    pub(crate) attachment_previews: HashMap<String, Arc<gpui::Image>>,
    pub(crate) attachment_load_id: Option<String>,
}

fn multiline_input(
    value: impl Into<String>,
    placeholder: impl Into<gpui::SharedString>,
    max_rows: usize,
    window: &mut Window,
    cx: &mut Context<TextareaState>,
) -> TextareaState {
    let mut input = TextareaState::new(window, cx)
        .auto_grow(1, max_rows)
        .soft_wrap(true)
        .placeholder(placeholder);
    input.insert(value.into(), window, cx);
    input
}

struct CachedMarkdown {
    source: String,
    document: MarkdownDocument,
}

#[derive(Clone)]
pub(crate) struct PendingTitleTransition {
    pub(crate) old_title: String,
    pub(crate) new_title: String,
}

pub(crate) struct TitleTransition {
    old_graphemes: Vec<String>,
    new_graphemes: Vec<String>,
    new_title: String,
    started_at: Instant,
}

impl TitleTransition {
    pub(crate) fn new(old_title: &str, new_title: &str) -> Self {
        Self {
            old_graphemes: old_title.graphemes(true).map(str::to_string).collect(),
            new_graphemes: new_title.graphemes(true).map(str::to_string).collect(),
            new_title: new_title.to_string(),
            started_at: Instant::now(),
        }
    }

    pub(crate) fn frame(&self) -> (String, bool) {
        self.frame_at(self.started_at.elapsed())
    }

    fn frame_at(&self, elapsed: Duration) -> (String, bool) {
        const CHARACTER_INTERVAL_MS: u128 = 30;
        let step = elapsed.as_millis() / CHARACTER_INTERVAL_MS;
        let step = usize::try_from(step).unwrap_or(usize::MAX);
        if step < self.old_graphemes.len() {
            return (
                self.old_graphemes[..self.old_graphemes.len() - step].concat(),
                false,
            );
        }

        let written = (step - self.old_graphemes.len()).min(self.new_graphemes.len());
        (
            self.new_graphemes[..written].concat(),
            written == self.new_graphemes.len(),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DefaultModelRole {
    Primary,
    TitleGeneration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FontRole {
    Ui,
    Code,
}

impl FontRole {
    fn default_family(self) -> &'static str {
        match self {
            Self::Ui => crate::domain::DEFAULT_UI_FONT_FAMILY,
            Self::Code => crate::domain::DEFAULT_CODE_FONT_FAMILY,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum SystemPromptMode {
    #[default]
    Compact,
    Expanded,
    Editing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PaletteCommand {
    NewConversation,
    ChooseModel,
    FocusConversationSearch,
    ToggleSidebar,
    ToggleInspector,
    EditSystemPrompt,
    OpenChat,
    OpenTextToSpeech,
    OpenSettings,
}

impl PaletteCommand {
    pub(crate) const ALL: [Self; 9] = [
        Self::NewConversation,
        Self::ChooseModel,
        Self::FocusConversationSearch,
        Self::ToggleSidebar,
        Self::ToggleInspector,
        Self::EditSystemPrompt,
        Self::OpenChat,
        Self::OpenTextToSpeech,
        Self::OpenSettings,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::NewConversation => "New conversation",
            Self::ChooseModel => "Choose model",
            Self::FocusConversationSearch => "Search conversations",
            Self::ToggleSidebar => "Toggle sidebar",
            Self::ToggleInspector => "Toggle Inspector",
            Self::EditSystemPrompt => "Edit System Prompt",
            Self::OpenChat => "Open chat",
            Self::OpenTextToSpeech => "Open Text to Speech",
            Self::OpenSettings => "Open settings",
        }
    }

    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::NewConversation => "Start a local conversation",
            Self::ChooseModel => "Choose a conversation model or change the primary model",
            Self::FocusConversationSearch => "Filter conversations by title",
            Self::ToggleSidebar => "Expand or collapse conversation navigation",
            Self::ToggleInspector => "Show or hide model, context, and request info",
            Self::EditSystemPrompt => "Customize instructions for this conversation",
            Self::OpenChat => "Return to the current conversation",
            Self::OpenTextToSpeech => "Open the audio.cpp TTS Playground",
            Self::OpenSettings => "Manage providers, models, and appearance",
        }
    }

    fn keywords(self) -> &'static str {
        match self {
            Self::NewConversation => "new create conversation chat",
            Self::ChooseModel => "model provider llm select choose",
            Self::FocusConversationSearch => "search find conversation title",
            Self::ToggleSidebar => "sidebar navigation collapse expand",
            Self::ToggleInspector => "inspector parameters context info",
            Self::EditSystemPrompt => "system prompt instructions edit",
            Self::OpenChat => "chat conversation messages",
            Self::OpenTextToSpeech => "tts text speech voice audio playground audio.cpp",
            Self::OpenSettings => "settings provider model appearance preferences",
        }
    }

    pub(crate) fn matches(self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || self.label().to_lowercase().contains(&query)
            || self.keywords().contains(&query)
    }
}

#[cfg(test)]
mod palette_command_tests {
    use super::PaletteCommand;

    #[test]
    fn text_to_speech_is_discoverable_by_label_and_audio_keywords() {
        assert!(PaletteCommand::ALL.contains(&PaletteCommand::OpenTextToSpeech));
        assert!(PaletteCommand::OpenTextToSpeech.matches("text to speech"));
        assert!(PaletteCommand::OpenTextToSpeech.matches("voice"));
        assert!(PaletteCommand::OpenTextToSpeech.matches("audio.cpp"));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingFocus {
    Root,
    ConversationSearch,
    SystemPrompt,
    SettingsPrompt,
    MessageEditor,
    Composer,
}

pub(crate) const COLLAPSED_THINKING_HEIGHT: f32 = 132.0;

pub struct OneChat {
    pub(crate) root_focus: FocusHandle,
    services: Services,
    pub(crate) data: DataState,
    pub(crate) mcp: McpState,
    pub(crate) navigation: NavigationState,
    pub(crate) sidebar: SidebarState,
    pub(crate) overlays: OverlayState,
    pub(crate) playback: PlaybackState,
    pub(crate) chat: ChatState,
    pub(crate) tts: TtsState,
    pub(crate) settings_ui: SettingsState,
    pub(crate) applied_component_theme: Option<(gpui_component::ThemeMode, String)>,
}

impl OneChat {
    pub fn initial_focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.root_focus.clone()
    }
}

impl Render for OneChat {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        shell::render(self, window, cx)
    }
}
