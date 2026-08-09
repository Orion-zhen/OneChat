use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
    time::Instant,
};

use gpui::{Entity, ScrollHandle, Task};
use gpui_component::{
    input::InputState, list::ListState, select::SelectState, slider::SliderState,
};
use tokio::runtime::Runtime;

use super::{
    CachedMarkdown, ConnectionTestStatus, DestructiveAction, MessageEditor, Page, PendingFocus,
    PendingTitleTransition, RenameEditor, SystemPromptMode, TitleTransition,
    motion::{DrawerMotion, MessageScrollMotion, ThinkingMotion, VisibilityMotion},
};
use crate::{
    application::generation::GenerationManager,
    desktop::ui::{
        inspector::{GenerationConfigEditor, InspectorTab},
        selectable_text::TextSelection,
        settings::{
            DefaultModelItem, FontFamilyItem, McpServerEditor, ModelEditor, PromptPresetEditor,
            PromptSelectItem, ProviderEditor, SearchableItems, SettingsSection,
        },
        shell::{
            CommandPaletteDelegate, ModelPickerDelegate, PromptPickerDelegate,
            ReasoningPickerDelegate,
        },
    },
    domain::AttachmentDraft,
    mcp::{McpManager, McpSnapshot},
    storage::{Storage, StorageSnapshot},
};

pub(super) struct Services {
    pub(super) storage: Arc<Storage>,
    pub(super) runtime: Arc<Runtime>,
    pub(super) mcp: Arc<McpManager>,
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
    pub(crate) sidebar_motion: DrawerMotion,
    pub(crate) inspector_motion: DrawerMotion,
    pub(crate) inspector_pointer: InspectorPointerState,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum InspectorPointerState {
    #[default]
    Idle,
    PressedOutside,
}

impl InspectorPointerState {
    pub(crate) fn begin_outside(&mut self) {
        *self = Self::PressedOutside;
    }

    pub(crate) fn cancel(&mut self) {
        *self = Self::Idle;
    }

    pub(crate) fn release_outside(&mut self) -> bool {
        std::mem::take(self) == Self::PressedOutside
    }
}

pub(crate) struct SidebarState {
    pub(crate) search_input: Entity<InputState>,
    pub(crate) hovered_conversation_id: Option<String>,
    pub(super) rename_editor: Option<RenameEditor>,
}

pub(crate) struct OverlayState {
    pub(crate) command_picker: Entity<ListState<CommandPaletteDelegate>>,
    pub(crate) model_picker: Entity<ListState<ModelPickerDelegate>>,
    pub(crate) prompt_picker: Entity<ListState<PromptPickerDelegate>>,
    pub(crate) reasoning_picker: Entity<ListState<ReasoningPickerDelegate>>,
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
    pub(crate) thinking_scrolls: HashMap<String, ScrollHandle>,
    pub(crate) thinking_motions: HashMap<String, ThinkingMotion>,
    pub(crate) thinking_started_at: HashMap<String, Instant>,
    pub(crate) follow_latest: bool,
    pub(crate) system_prompt_mode: SystemPromptMode,
    pub(crate) system_prompt_editor: Option<Entity<InputState>>,
    pub(crate) generation_config_editor: Option<GenerationConfigEditor>,
    pub(crate) generation_config_save_revision: u64,
    pub(crate) parameter_error: Option<String>,
    pub(crate) composer: Entity<InputState>,
    pub(crate) composer_multiline: Cell<bool>,
    pub(crate) composer_expanded: Cell<bool>,
    pub(crate) attachments: Vec<AttachmentDraft>,
    pub(crate) attachment_previews: HashMap<String, Arc<gpui::Image>>,
    pub(crate) attachments_loading: bool,
    pub(crate) attachments_revision: u64,
    pub(super) generations: GenerationManager,
    pub(super) markdown_documents: HashMap<String, CachedMarkdown>,
    pub(super) pending_title_transitions: HashMap<String, PendingTitleTransition>,
    pub(super) title_transitions: HashMap<String, TitleTransition>,
}

pub(crate) struct SettingsState {
    pub(crate) section: SettingsSection,
    pub(crate) ui_font_select: Entity<SelectState<SearchableItems<FontFamilyItem>>>,
    pub(crate) code_font_select: Entity<SelectState<SearchableItems<FontFamilyItem>>>,
    pub(crate) message_font_size_slider: Entity<SliderState>,
    pub(crate) background_opacity_slider: Entity<SliderState>,
    pub(crate) message_width_slider: Entity<SliderState>,
    pub(crate) primary_model_select: Entity<SelectState<Vec<DefaultModelItem>>>,
    pub(crate) title_model_select: Entity<SelectState<Vec<DefaultModelItem>>>,
    pub(crate) default_prompt_select: Entity<SelectState<Vec<PromptSelectItem>>>,
    pub(crate) synced_primary_models: Vec<DefaultModelItem>,
    pub(crate) synced_title_models: Vec<DefaultModelItem>,
    pub(crate) synced_prompts: Vec<PromptSelectItem>,
    pub(crate) viewed_prompt_preset: Option<String>,
    pub(crate) prompt_preset_editor: Option<PromptPresetEditor>,
    pub(crate) title_prompt_editor: Option<Entity<InputState>>,
    pub(crate) mcp_json_import: Entity<InputState>,
    pub(crate) mcp_server_editor: Option<McpServerEditor>,
    pub(crate) mcp_error: Option<String>,
    pub(crate) expanded_mcp_server_ids: HashSet<String>,
    pub(crate) mcp_connection_tests: BTreeMap<String, ConnectionTestStatus>,
    pub(crate) connection_tests: BTreeMap<String, ConnectionTestStatus>,
    pub(crate) provider_editor: Option<ProviderEditor>,
    pub(crate) model_editor: Option<ModelEditor>,
    pub(super) model_fetch_revision: u64,
    pub(crate) form_error: Option<String>,
}
