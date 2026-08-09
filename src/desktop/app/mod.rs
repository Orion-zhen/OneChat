mod attachments;
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
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    App, ClipboardItem, Context, Entity, FocusHandle, Focusable as _, Render, ScrollHandle,
    ScrollWheelEvent, Task, Window, prelude::*, px,
};
use gpui_component::{
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    combobox::ComboboxEvent,
    dialog::DialogFooter,
    input::{InputEvent, InputState},
    list::{ListEvent, ListState},
    select::{SelectEvent, SelectState},
    slider::{SliderEvent, SliderState},
};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    application::{
        generation::{
            GenerationManager, GenerationStart, GenerationUpdate, PreparedGeneration,
            run_generation,
        },
        title::generate_title,
    },
    desktop::ui::{
        inspector::{
            GenerationConfigEditor, GenerationParameter, GenerationParameterItem, InspectorTab,
        },
        selectable_text::TextSelection,
        settings::{
            Capability, DefaultModelItem, FontFamilyItem, McpServerEditor, McpServerEditorMode,
            McpServerTransportEditor, ModelEditor, PromptPresetEditor, PromptSelectItem,
            ProviderEditor, SearchableItems, SettingsSection,
        },
        shell::{self, CommandPaletteDelegate, ModelPickerDelegate, PromptPickerDelegate},
        stream::follow_after_scroll,
    },
    domain::{
        AppSettings, AssistantResponse, AttachmentDraft, AttachmentDraftFile, AttachmentKind,
        AutoTitleState, Conversation, DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT, Message,
        MessageStatus, Model, Provider, RequestInfo, SystemPromptPreset, Theme, ToolRef,
        ToolSelection, Turn, active_turns, new_id, now_timestamp, user_branches,
    },
    markdown::MarkdownDocument,
    mcp::{McpConfig, McpManager, McpServerConfig, McpSnapshot},
    providers::{self, AvailableModel},
    storage::{Storage, StorageError, StorageResult, StorageSnapshot},
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
            Self::DeleteMcpServer { .. } => "Delete MCP Server?",
            Self::ClearContext { .. } => "Clear Conversation?",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::ClearContext { .. } => {
                "This removes the conversation context used for future responses."
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

struct MessageEditor {
    target: MessageEditorTarget,
    input: Entity<InputState>,
}

fn is_composer_submit(event: &InputEvent) -> bool {
    matches!(event, InputEvent::PressEnter { shift: false, .. })
}

fn multiline_input(
    value: impl Into<String>,
    placeholder: impl Into<gpui::SharedString>,
    window: &mut Window,
    cx: &mut Context<InputState>,
) -> InputState {
    let value = value.into();
    let cursor = value.len();
    let mut input = InputState::new(window, cx)
        .auto_grow(1, 8)
        .soft_wrap(true)
        .placeholder(placeholder)
        .default_value(value);
    input.set_selected_range(cursor..cursor, cx);
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

    pub(crate) fn matches(self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || self.label().to_lowercase().contains(&query)
            || self.keywords().contains(&query)
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
    pub(crate) chat: ChatState,
    pub(crate) settings_ui: SettingsState,
    pub(crate) applied_component_theme: Option<gpui_component::ThemeMode>,
}

impl OneChat {
    pub fn new(
        storage: Arc<Storage>,
        runtime: Arc<Runtime>,
        mcp: Arc<McpManager>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let root_focus = cx.focus_handle();
        let timeline_focus = cx.focus_handle().tab_stop(true);
        let applied_component_theme = Some(crate::desktop::ui::theme::component_mode(
            Theme::System,
            window.appearance(),
        ));
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        cx.subscribe(&search_input, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();

        let composer = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(1, 8)
                .soft_wrap(true)
                .submit_on_enter(true)
                .placeholder("Message")
        });
        cx.subscribe_in(
            &composer,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if is_composer_submit(event) {
                    this.send_composer(window, cx);
                } else if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        )
        .detach();

        let command_picker =
            cx.new(|cx| ListState::new(CommandPaletteDelegate::new(), window, cx).searchable(true));
        cx.subscribe_in(
            &command_picker,
            window,
            |this, picker, event: &ListEvent, window, cx| {
                let ListEvent::Confirm(index) = event else {
                    return;
                };
                let command = picker.read(cx).delegate().command(*index);
                if let Some(command) = command {
                    window.close_dialog(cx);
                    this.execute_command(command, window, cx);
                }
            },
        )
        .detach();

        let model_picker =
            cx.new(|cx| ListState::new(ModelPickerDelegate::empty(), window, cx).searchable(true));
        cx.subscribe_in(
            &model_picker,
            window,
            |this, picker, event: &ListEvent, window, cx| {
                let ListEvent::Confirm(index) = event else {
                    return;
                };
                let model_id = picker.read(cx).delegate().selected_model_id(*index);
                if let Some(model_id) = model_id {
                    window.close_dialog(cx);
                    this.select_model(model_id, cx);
                }
            },
        )
        .detach();

        let prompt_picker =
            cx.new(|cx| ListState::new(PromptPickerDelegate::empty(), window, cx).searchable(true));
        cx.subscribe_in(
            &prompt_picker,
            window,
            |this, picker, event: &ListEvent, window, cx| {
                let ListEvent::Confirm(index) = event else {
                    return;
                };
                let name = picker.read(cx).delegate().selected_name(*index);
                if let Some(name) = name {
                    window.close_dialog(cx);
                    this.select_system_prompt_preset(name, cx);
                }
            },
        )
        .detach();

        let message_width_slider = cx.new(|_| {
            SliderState::new()
                .min(crate::domain::MIN_MESSAGE_WIDTH_RATIO)
                .max(crate::domain::MAX_MESSAGE_WIDTH_RATIO)
                .step(0.01)
                .default_value(AppSettings::default().message_width_ratio())
        });
        cx.subscribe(
            &message_width_slider,
            |this, _, event: &SliderEvent, cx| match event {
                SliderEvent::Change(value) => {
                    this.update_message_width_ratio(value.start(), cx);
                }
                SliderEvent::Release(value) => {
                    this.update_message_width_ratio(value.start(), cx);
                    this.save_settings(cx);
                }
            },
        )
        .detach();

        let message_font_size_slider = cx.new(|_| {
            SliderState::new()
                .min(crate::domain::MIN_MESSAGE_FONT_SIZE)
                .max(crate::domain::MAX_MESSAGE_FONT_SIZE)
                .step(1.0)
                .default_value(AppSettings::default().message_font_size())
        });
        cx.subscribe(
            &message_font_size_slider,
            |this, _, event: &SliderEvent, cx| match event {
                SliderEvent::Change(value) => {
                    this.update_message_font_size(value.start(), cx);
                }
                SliderEvent::Release(value) => {
                    this.update_message_font_size(value.start(), cx);
                    this.save_settings(cx);
                }
            },
        )
        .detach();

        let background_opacity_slider = cx.new(|_| {
            SliderState::new()
                .min(crate::domain::MIN_BACKGROUND_OPACITY)
                .max(crate::domain::MAX_BACKGROUND_OPACITY)
                .step(0.01)
                .default_value(AppSettings::default().background_opacity())
        });
        cx.subscribe(
            &background_opacity_slider,
            |this, _, event: &SliderEvent, cx| match event {
                SliderEvent::Change(value) => {
                    this.update_background_opacity(value.start(), cx);
                }
                SliderEvent::Release(value) => {
                    this.update_background_opacity(value.start(), cx);
                    this.save_settings(cx);
                }
            },
        )
        .detach();

        let primary_model_select =
            cx.new(|cx| SelectState::new(Vec::<DefaultModelItem>::new(), None, window, cx));
        cx.subscribe(
            &primary_model_select,
            |this, _, event: &SelectEvent<Vec<DefaultModelItem>>, cx| {
                let SelectEvent::Confirm(value) = event;
                this.select_default_model(DefaultModelRole::Primary, value.clone().flatten(), cx);
            },
        )
        .detach();

        let title_model_select =
            cx.new(|cx| SelectState::new(Vec::<DefaultModelItem>::new(), None, window, cx));
        cx.subscribe(
            &title_model_select,
            |this, _, event: &SelectEvent<Vec<DefaultModelItem>>, cx| {
                let SelectEvent::Confirm(value) = event;
                this.select_default_model(
                    DefaultModelRole::TitleGeneration,
                    value.clone().flatten(),
                    cx,
                );
            },
        )
        .detach();

        let default_prompt_select =
            cx.new(|cx| SelectState::new(Vec::<PromptSelectItem>::new(), None, window, cx));
        cx.subscribe(
            &default_prompt_select,
            |this, _, event: &SelectEvent<Vec<PromptSelectItem>>, cx| {
                let SelectEvent::Confirm(value) = event;
                this.select_default_prompt(value.clone().flatten(), cx);
            },
        )
        .detach();

        let font_items = cx
            .text_system()
            .all_font_names()
            .into_iter()
            .filter(|family| family != crate::desktop::ui::icons::LUCIDE_FONT_FAMILY)
            .map(FontFamilyItem::new)
            .collect::<Vec<_>>();
        let ui_font_select = cx.new(|cx| {
            SelectState::new(SearchableItems::new(font_items.clone()), None, window, cx)
                .searchable(true)
        });
        cx.subscribe_in(
            &ui_font_select,
            window,
            |this, select, event: &SelectEvent<SearchableItems<FontFamilyItem>>, window, cx| {
                let SelectEvent::Confirm(Some(family)) = event else {
                    return;
                };
                this.add_font_family(FontRole::Ui, family.clone(), cx);
                select.update(cx, |select, cx| select.set_selected_index(None, window, cx));
            },
        )
        .detach();
        let code_font_select = cx.new(|cx| {
            SelectState::new(SearchableItems::new(font_items), None, window, cx).searchable(true)
        });
        cx.subscribe_in(
            &code_font_select,
            window,
            |this, select, event: &SelectEvent<SearchableItems<FontFamilyItem>>, window, cx| {
                let SelectEvent::Confirm(Some(family)) = event else {
                    return;
                };
                this.add_font_family(FontRole::Code, family.clone(), cx);
                select.update(cx, |select, cx| select.set_selected_index(None, window, cx));
            },
        )
        .detach();

        let mcp_json_import = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .soft_wrap(true)
                .placeholder("Paste a JSON or JSONC object containing mcpServers")
        });
        let text_selection = TextSelection::new(cx.focus_handle());
        let mcp_snapshot = McpSnapshot::empty(mcp.config_path());
        let mut this = Self {
            root_focus,
            services: Services {
                storage,
                runtime,
                mcp,
            },
            data: DataState {
                snapshot: StorageSnapshot::default(),
                loading: true,
                error: None,
                storage_task: Task::ready(()),
            },
            mcp: McpState {
                snapshot: mcp_snapshot,
                loading: false,
            },
            navigation: NavigationState {
                page: Page::Chat,
                inspector_open: false,
                inspector_tab: InspectorTab::default(),
                pending_focus: None,
                sidebar_motion: DrawerMotion::new(true),
                inspector_motion: DrawerMotion::new(false),
                inspector_pointer: InspectorPointerState::default(),
            },
            sidebar: SidebarState {
                search_input,
                hovered_conversation_id: None,
                rename_editor: None,
            },
            overlays: OverlayState {
                command_picker,
                model_picker,
                prompt_picker,
                response_model_turn_id: None,
                destructive_action: None,
            },
            chat: ChatState {
                draft_model_id: None,
                selected_request_id: None,
                visible_response_ids: HashMap::new(),
                expanded_error_ids: HashSet::new(),
                thinking_expansion_overrides: HashSet::new(),
                expanded_tool_execution_ids: HashSet::new(),
                expanded_conversation_tool_server_ids: HashSet::new(),
                message_editor: None,
                message_scroll: ScrollHandle::new(),
                message_scroll_motion: MessageScrollMotion::new(),
                jump_to_latest_motion: VisibilityMotion::new(false),
                timeline: TimelineState {
                    focus: timeline_focus,
                    hovered: false,
                    pointer_y: None,
                    active_item: None,
                    expansion_motion: VisibilityMotion::new(false),
                },
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
                attachments: Vec::new(),
                attachment_previews: HashMap::new(),
                attachments_loading: false,
                attachments_revision: 0,
                generations: GenerationManager::default(),
                markdown_documents: HashMap::new(),
                pending_title_transitions: HashMap::new(),
                title_transitions: HashMap::new(),
            },
            settings_ui: SettingsState {
                section: SettingsSection::default(),
                ui_font_select,
                code_font_select,
                message_font_size_slider,
                background_opacity_slider,
                message_width_slider,
                primary_model_select,
                title_model_select,
                default_prompt_select,
                synced_primary_models: Vec::new(),
                synced_title_models: Vec::new(),
                synced_prompts: Vec::new(),
                viewed_prompt_preset: None,
                prompt_preset_editor: None,
                title_prompt_editor: None,
                mcp_json_import,
                mcp_server_editor: None,
                mcp_error: None,
                expanded_mcp_server_ids: HashSet::new(),
                mcp_connection_tests: BTreeMap::new(),
                connection_tests: BTreeMap::new(),
                provider_editor: None,
                model_editor: None,
                model_fetch_revision: 0,
                form_error: None,
            },
            applied_component_theme,
        };
        this.load_startup_snapshot(cx);
        this.reload_mcp(cx);
        this
    }

    pub fn initial_focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.root_focus.clone()
    }
}

impl Render for OneChat {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        shell::render(self, window, cx)
    }
}
