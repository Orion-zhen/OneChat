mod chat;
mod conversations;
mod data;
mod generation;
mod messages;
mod navigation;
mod overlays;
mod settings;
mod state;

pub use navigation::{ConversationGroup, Page};
use state::*;

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    ClipboardItem, Context, Entity, FocusHandle, Render, ScrollHandle, ScrollWheelEvent, Task,
    Window, ease_out_quint, prelude::*,
};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

use crate::{
    application::generation::{
        GenerationManager, GenerationStart, GenerationUpdate, PreparedGeneration, run_generation,
    },
    desktop::ui::{
        composer::{Composer, ComposerEvent, PickerDirection},
        inspector::{GenerationConfigEditor, GenerationParameter, InspectorTab},
        selectable_text::TextSelection,
        settings::{Capability, ModelEditor, ProviderEditor, SettingsSection},
        shell,
        stream::follow_after_scroll,
    },
    domain::{
        AppSettings, AssistantResponse, ChatMessage, Conversation, MessageStatus, Model, Provider,
        ProviderKind, RequestInfo, SystemPromptSource, Theme, Turn, active_turns, now_timestamp,
        user_branches,
    },
    markdown::MarkdownDocument,
    providers::{self, AvailableModel},
    storage::{Storage, StorageResult, StorageSnapshot},
};

#[derive(Clone, Debug)]
pub(crate) enum ConnectionTestStatus {
    Testing,
    Connected,
    Failed(String),
}

#[derive(Clone, Debug)]
pub(crate) enum DestructiveAction {
    DeleteConversation { id: String, title: String },
    DeleteProvider { id: String, name: String },
    DeleteModel { id: String, name: String },
    ClearContext { conversation_id: String },
}

struct RenameEditor {
    conversation_id: String,
    input: Entity<Composer>,
}

enum MessageEditorTarget {
    User(String),
    Assistant(String),
}

struct MessageEditor {
    target: MessageEditorTarget,
    input: Entity<Composer>,
}

struct CachedMarkdown {
    source: String,
    document: MarkdownDocument,
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
    OpenSettings,
}

impl PaletteCommand {
    pub(crate) const ALL: [Self; 8] = [
        Self::NewConversation,
        Self::ChooseModel,
        Self::FocusConversationSearch,
        Self::ToggleSidebar,
        Self::ToggleInspector,
        Self::EditSystemPrompt,
        Self::OpenChat,
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
            Self::OpenSettings => "settings provider model appearance preferences",
        }
    }

    fn matches(self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || self.label().to_lowercase().contains(&query)
            || self.keywords().contains(&query)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingFocus {
    Root,
    CommandPalette,
    ModelPicker,
    ConversationSearch,
    SystemPrompt,
    DefaultSystemPrompt,
    MessageEditor,
    Composer,
}

pub(crate) const COLLAPSED_THINKING_HEIGHT: f32 = 132.0;

pub struct OneChat {
    pub(crate) root_focus: FocusHandle,
    services: Services,
    pub(crate) data: DataState,
    pub(crate) navigation: NavigationState,
    pub(crate) sidebar: SidebarState,
    pub(crate) overlays: OverlayState,
    pub(crate) chat: ChatState,
    pub(crate) settings_ui: SettingsState,
}

impl OneChat {
    pub fn new(storage: Arc<Storage>, runtime: Arc<Runtime>, cx: &mut Context<Self>) -> Self {
        let root_focus = cx.focus_handle();
        let search_input = cx.new(|cx| Composer::single_line("", "Search conversations", cx));
        cx.subscribe(&search_input, |this, _, event, cx| {
            if let ComposerEvent::Changed(query) = event {
                this.sidebar.search_query = query.clone();
                cx.notify();
            }
        })
        .detach();

        let composer = cx.new(Composer::new);
        cx.subscribe(&composer, |this, _, event, cx| {
            if let ComposerEvent::Submit(prompt) = event {
                this.start_generation(prompt.clone(), cx);
            }
        })
        .detach();

        let command_input = cx.new(|cx| Composer::picker("Type a command…", cx));
        cx.subscribe(&command_input, |this, _, event, cx| match event {
            ComposerEvent::Changed(query) => {
                this.overlays.command_query = query.clone();
                this.overlays.command_selection = 0;
                this.overlays.command_scroll.scroll_to_item(0);
                cx.notify();
            }
            ComposerEvent::Submit(_) => this.confirm_command(cx),
            ComposerEvent::Navigate(direction) => this.navigate_command(*direction, cx),
            ComposerEvent::Cancel => this.close_command_palette(cx),
        })
        .detach();

        let model_search_input = cx.new(|cx| Composer::picker("Search models…", cx));
        cx.subscribe(&model_search_input, |this, _, event, cx| match event {
            ComposerEvent::Changed(query) => {
                this.overlays.model_query = query.clone();
                this.overlays.model_selection = this.initial_model_selection();
                this.overlays
                    .model_scroll
                    .scroll_to_item(this.overlays.model_selection);
                cx.notify();
            }
            ComposerEvent::Submit(_) => this.confirm_model(cx),
            ComposerEvent::Navigate(direction) => this.navigate_model(*direction, cx),
            ComposerEvent::Cancel => this.close_model_picker(cx),
        })
        .detach();

        let text_selection = TextSelection::new(cx.focus_handle());
        let mut this = Self {
            root_focus,
            services: Services { storage, runtime },
            data: DataState {
                snapshot: StorageSnapshot::default(),
                loading: true,
                error: None,
                storage_task: Task::ready(()),
            },
            navigation: NavigationState {
                page: Page::Chat,
                inspector_open: false,
                inspector_tab: InspectorTab::default(),
                pending_focus: None,
                inspector_motion: DrawerMotion::new(false),
            },
            sidebar: SidebarState {
                search_query: String::new(),
                search_input,
                rename_editor: None,
            },
            overlays: OverlayState {
                command_palette_open: false,
                command_query: String::new(),
                command_input,
                command_selection: 0,
                command_scroll: ScrollHandle::new(),
                model_picker_open: false,
                response_model_turn_id: None,
                destructive_action: None,
                model_query: String::new(),
                model_search_input,
                model_selection: 0,
                model_scroll: ScrollHandle::new(),
            },
            chat: ChatState {
                draft_model_id: None,
                selected_request_id: None,
                visible_response_ids: HashMap::new(),
                expanded_error_ids: HashSet::new(),
                thinking_expansion_overrides: HashSet::new(),
                message_editor: None,
                message_scroll: ScrollHandle::new(),
                text_selection,
                thinking_scrolls: HashMap::new(),
                thinking_motions: HashMap::new(),
                thinking_started_at: HashMap::new(),
                follow_latest: true,
                system_prompt_mode: SystemPromptMode::default(),
                system_prompt_editor: None,
                generation_config_editor: None,
                generation_config_save_revision: 0,
                parameter_error: None,
                composer,
                generations: GenerationManager::default(),
                markdown_documents: HashMap::new(),
            },
            settings_ui: SettingsState {
                section: SettingsSection::default(),
                message_width_dragging: false,
                default_model_menu_open: false,
                default_system_prompt_editor: None,
                connection_tests: BTreeMap::new(),
                provider_editor: None,
                model_editor: None,
                model_fetch_revision: 0,
                form_error: None,
            },
        };
        this.load_startup_snapshot(cx);
        this
    }

    pub fn initial_focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.root_focus.clone()
    }
}

fn moved_selection(current: usize, len: usize, direction: PickerDirection) -> usize {
    if len == 0 {
        return 0;
    }
    match direction {
        PickerDirection::Previous => current.checked_sub(1).unwrap_or(len - 1),
        PickerDirection::Next => (current + 1) % len,
    }
}

impl Render for OneChat {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        shell::render(self, window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_filtering_uses_labels_and_keywords() {
        assert_eq!(
            PaletteCommand::ALL
                .into_iter()
                .filter(|command| command.matches("provider"))
                .collect::<Vec<_>>(),
            vec![PaletteCommand::ChooseModel, PaletteCommand::OpenSettings]
        );
        assert!(
            PaletteCommand::ALL
                .into_iter()
                .all(|command| command.matches(""))
        );
    }

    #[test]
    fn picker_navigation_wraps_and_handles_empty_results() {
        assert_eq!(moved_selection(0, 3, PickerDirection::Previous), 2);
        assert_eq!(moved_selection(2, 3, PickerDirection::Next), 0);
        assert_eq!(moved_selection(4, 0, PickerDirection::Next), 0);
    }
}
