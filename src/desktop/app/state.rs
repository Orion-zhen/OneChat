use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
    time::Instant,
};

use gpui::{Entity, FocusHandle, ScrollHandle, Task};
use gpui_component::{
    input::{InputState, TextareaState},
    list::ListState,
    select::SelectState,
    slider::SliderState,
};
use tokio::runtime::Runtime;

use super::{
    CachedMarkdown, ConnectionTestStatus, DestructiveAction, MessageEditor, Page, PendingFocus,
    PendingTitleTransition, RenameEditor, SystemPromptMode, TitleTransition,
    motion::{
        DrawerMotion, MessageScrollMotion, SidebarWidthMotion, ThinkingMotion, VisibilityMotion,
    },
};
use crate::{
    application::generation::GenerationManager,
    desktop::{
        audio_playback::{AudioPlayback, PlaybackSnapshot},
        audio_recording::{AudioRecording, RecordingSnapshot},
        branch_swipe::{BranchSwipeState, BranchSwipeTarget},
        ui::{
            inspector::{GenerationConfigEditor, InspectorTab},
            selectable_text::TextSelection,
            settings::{
                DefaultModelItem, FontFamilyItem, McpServerEditor, ModelEditor,
                PromptPresetWorkspace, PromptSelectItem, PromptVariableEditor, ProviderEditor,
                ReasoningPresetSelectItem, SearchableItems, SettingsSection,
            },
            shell::{
                CommandPaletteDelegate, ModelPickerDelegate, PromptPickerDelegate,
                ReasoningPickerDelegate,
            },
            stream::HorizontalScrollRegistry,
        },
    },
    domain::AttachmentDraft,
    mcp::{McpManager, McpSnapshot},
    storage::{Storage, StorageSnapshot},
};

#[cfg(target_os = "macos")]
use crate::{desktop::pressure_touch::ForceClickGesture, domain::Turn};

pub(super) struct Services {
    pub(super) storage: Arc<Storage>,
    pub(super) runtime: Arc<Runtime>,
    pub(super) mcp: Arc<McpManager>,
    pub(super) audio_playback: AudioPlayback,
    pub(super) audio_recording: AudioRecording,
}

pub(crate) struct DataState {
    pub(crate) snapshot: StorageSnapshot,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(super) storage_task: Task<()>,
}

pub(crate) struct McpState {
    pub(crate) snapshot: McpSnapshot,
    pub(crate) loading: bool,
}

pub(crate) struct NavigationState {
    pub(crate) page: Page,
    pub(crate) inspector_open: bool,
    pub(crate) inspector_tab: InspectorTab,
    pub(crate) pending_focus: Option<PendingFocus>,
    pub(crate) sidebar_width_motion: SidebarWidthMotion,
    pub(crate) inspector_motion: DrawerMotion,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Debug, Default)]
pub(crate) enum ConversationPeekContent {
    #[default]
    Loading,
    Ready(Vec<Turn>),
    Failed,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Debug, Default)]
pub(crate) struct ConversationPeekState {
    pub(crate) conversation_id: Option<String>,
    pub(crate) anchor_y: f32,
    pub(crate) content: ConversationPeekContent,
    pub(super) revision: u64,
    pub(crate) force_click: ForceClickGesture<String>,
}

pub(crate) struct SidebarState {
    pub(crate) width: f32,
    pub(crate) search_input: Entity<InputState>,
    pub(crate) hovered_conversation_id: Option<String>,
    pub(crate) generation_border_epoch: Instant,
    pub(crate) unseen_generations: HashMap<String, UnseenGeneration>,
    #[cfg(target_os = "macos")]
    pub(crate) conversation_peek: ConversationPeekState,
    pub(super) rename_editor: Option<RenameEditor>,
    #[cfg(target_os = "macos")]
    pub(crate) rename_force_click: crate::desktop::pressure_touch::ForceClickState,
    #[cfg(target_os = "macos")]
    pub(crate) force_renamed_conversation_id: Option<String>,
}

#[derive(Clone)]
pub(crate) struct UnseenGeneration {
    pub(crate) request_id: String,
    pub(crate) completion_phase: f32,
}

#[derive(Clone, Copy)]
pub(crate) struct GenerationBorderClock {
    epoch: Instant,
    offset: f32,
}

impl GenerationBorderClock {
    pub(crate) fn phase(self) -> f32 {
        (self.epoch.elapsed().as_secs_f32() / 1.8 + self.offset).fract()
    }
}

impl SidebarState {
    pub(crate) fn generation_border_clock(&self, conversation_id: &str) -> GenerationBorderClock {
        let hash = conversation_id
            .bytes()
            .fold(2_166_136_261_u32, |hash, byte| {
                (hash ^ u32::from(byte)).wrapping_mul(16_777_619)
            });
        GenerationBorderClock {
            epoch: self.generation_border_epoch,
            offset: hash as f32 / u32::MAX as f32,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PickerOverlay {
    Model,
    Prompt,
    Reasoning,
}

pub(crate) struct OverlayState {
    pub(crate) command_picker: Entity<ListState<CommandPaletteDelegate>>,
    pub(crate) model_picker: Entity<ListState<ModelPickerDelegate>>,
    pub(crate) prompt_picker: Entity<ListState<PromptPickerDelegate>>,
    pub(crate) reasoning_picker: Entity<ListState<ReasoningPickerDelegate>>,
    pub(crate) picker: Option<PickerOverlay>,
    pub(crate) picker_motion: VisibilityMotion,
    pub(crate) picker_previous_focus: Option<FocusHandle>,
    pub(crate) response_model_turn_id: Option<String>,
    pub(crate) destructive_action: Option<DestructiveAction>,
}

pub(crate) struct TimelineState {
    pub(crate) focus: gpui::FocusHandle,
    pub(crate) hovered: bool,
    pub(crate) pointer_y: Option<f32>,
    pub(crate) active_item: Option<usize>,
    pub(crate) expansion_motion: VisibilityMotion,
}

pub(crate) struct PlaybackState {
    pub(crate) snapshot: PlaybackSnapshot,
    pub(crate) seek_slider: Entity<SliderState>,
    pub(crate) seek_preview: Option<f32>,
    pub(crate) seek_target_ms: Option<u64>,
    pub(super) observer_task: Task<()>,
}

pub(crate) struct ChatState {
    pub(super) draft_model_id: Option<String>,
    pub(super) selected_request_id: Option<String>,
    pub(crate) visible_response_ids: HashMap<String, String>,
    pub(super) expanded_error_ids: HashSet<String>,
    pub(super) thinking_expansion_overrides: HashSet<String>,
    pub(super) expanded_tool_execution_ids: HashSet<String>,
    pub(crate) expanded_conversation_tool_server_ids: HashSet<String>,
    pub(super) message_editor: Option<MessageEditor>,
    pub(crate) message_scroll: ScrollHandle,
    pub(crate) message_scroll_motion: MessageScrollMotion,
    pub(crate) jump_to_latest_motion: VisibilityMotion,
    pub(crate) timeline: TimelineState,
    pub(crate) text_selection: TextSelection,
    pub(crate) branch_swipe: BranchSwipeState<BranchSwipeTarget>,
    #[cfg(target_os = "macos")]
    pub(crate) response_tab_force_click: ForceClickGesture<String>,
    pub(crate) horizontal_scrolls: HorizontalScrollRegistry,
    pub(crate) thinking_scrolls: HashMap<String, ScrollHandle>,
    pub(crate) thinking_motions: HashMap<String, ThinkingMotion>,
    pub(crate) thinking_started_at: HashMap<String, Instant>,
    pub(crate) follow_latest: bool,
    pub(crate) system_prompt_mode: SystemPromptMode,
    pub(crate) system_prompt_editor: Option<Entity<TextareaState>>,
    pub(crate) assistant_opening_editor: Option<Entity<TextareaState>>,
    pub(crate) generation_config_editor: Option<GenerationConfigEditor>,
    pub(crate) history_limit_slider: Entity<SliderState>,
    pub(crate) history_limit_preview: Option<crate::domain::HistoryLimit>,
    pub(crate) generation_config_save_revision: u64,
    pub(crate) parameter_error: Option<String>,
    pub(crate) composer: Entity<TextareaState>,
    pub(crate) composer_multiline: Cell<bool>,
    pub(crate) composer_expanded: Cell<bool>,
    pub(crate) context_usage_popover_open: bool,
    pub(crate) context_usage_popover_motion: VisibilityMotion,
    pub(crate) attachments: Vec<AttachmentDraft>,
    pub(crate) attachment_previews: HashMap<String, Arc<gpui::Image>>,
    pub(crate) attachments_loading: bool,
    pub(crate) attachments_revision: u64,
    pub(crate) audio_recording: RecordingSnapshot,
    pub(super) audio_recording_task: Task<()>,
    pub(super) recording_conversation_id: Option<String>,
    pub(super) generations: GenerationManager,
    pub(super) markdown_documents: HashMap<String, CachedMarkdown>,
    pub(super) pending_title_transitions: HashMap<String, PendingTitleTransition>,
    pub(super) title_transitions: HashMap<String, TitleTransition>,
}

#[derive(Clone, Debug)]
pub(crate) enum SettingsDestination {
    Section(SettingsSection),
    Page(Page),
    AddProvider,
}

pub(crate) struct SettingsState {
    pub(crate) section: SettingsSection,
    pub(crate) ui_font_select: Entity<SelectState<SearchableItems<FontFamilyItem>>>,
    pub(crate) code_font_select: Entity<SelectState<SearchableItems<FontFamilyItem>>>,
    pub(crate) theme_color: crate::desktop::ui::settings::ThemeColorControl,
    pub(crate) theme_color_save_revision: u64,
    pub(crate) message_font_size_slider: Entity<SliderState>,
    pub(crate) background_opacity_slider: Entity<SliderState>,
    pub(crate) message_width_slider: Entity<SliderState>,
    pub(crate) history_limit_slider: Entity<SliderState>,
    pub(crate) history_limit_save_pending: bool,
    pub(crate) primary_model_select: Entity<SelectState<Vec<DefaultModelItem>>>,
    pub(crate) title_model_select: Entity<SelectState<Vec<DefaultModelItem>>>,
    pub(crate) title_reasoning_select: Entity<SelectState<Vec<ReasoningPresetSelectItem>>>,
    pub(crate) default_prompt_select: Entity<SelectState<Vec<PromptSelectItem>>>,
    pub(crate) synced_primary_models: Vec<DefaultModelItem>,
    pub(crate) synced_title_models: Vec<DefaultModelItem>,
    pub(crate) synced_title_reasoning_presets: Vec<ReasoningPresetSelectItem>,
    pub(crate) synced_prompts: Vec<PromptSelectItem>,
    pub(crate) prompt_preset_workspace: Option<PromptPresetWorkspace>,
    pub(crate) pending_prompt_preset_exit: Option<SettingsDestination>,
    pub(crate) prompt_variable_editor: Option<PromptVariableEditor>,
    pub(crate) prompt_variable_test_revision: u64,
    pub(crate) prompt_builtins_expanded: bool,
    pub(crate) title_prompt_editor: Option<Entity<TextareaState>>,
    pub(crate) mcp_json_import: Entity<TextareaState>,
    pub(crate) mcp_server_editor: Option<McpServerEditor>,
    pub(crate) mcp_error: Option<String>,
    pub(crate) expanded_mcp_server_ids: HashSet<String>,
    pub(crate) mcp_connection_tests: BTreeMap<String, ConnectionTestStatus>,
    pub(crate) connection_tests: BTreeMap<String, ConnectionTestStatus>,
    pub(crate) provider_drop_target: Option<(String, bool)>,
    pub(crate) provider_editor: Option<ProviderEditor>,
    pub(crate) pending_provider_exit: Option<SettingsDestination>,
    pub(crate) model_editor: Option<ModelEditor>,
    pub(super) model_fetch_revision: u64,
    pub(crate) form_error: Option<String>,
}
