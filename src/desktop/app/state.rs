use super::*;

pub(super) struct Services {
    pub(super) storage: Arc<Storage>,
    pub(super) runtime: Arc<Runtime>,
}

pub(crate) struct DataState {
    pub(crate) snapshot: StorageSnapshot,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(super) storage_task: Task<()>,
}

pub(crate) struct NavigationState {
    pub(crate) page: Page,
    pub(crate) inspector_open: bool,
    pub(crate) inspector_tab: InspectorTab,
    pub(crate) pending_focus: Option<PendingFocus>,
}

pub(crate) struct SidebarState {
    pub(crate) search_query: String,
    pub(crate) search_input: Entity<Composer>,
    pub(super) rename_editor: Option<RenameEditor>,
}

pub(crate) struct OverlayState {
    pub(crate) command_palette_open: bool,
    pub(crate) command_query: String,
    pub(crate) command_input: Entity<Composer>,
    pub(crate) command_selection: usize,
    pub(crate) command_scroll: ScrollHandle,
    pub(crate) model_picker_open: bool,
    pub(crate) destructive_action: Option<DestructiveAction>,
    pub(crate) model_query: String,
    pub(crate) model_search_input: Entity<Composer>,
    pub(crate) model_selection: usize,
    pub(crate) model_scroll: ScrollHandle,
}

pub(crate) struct ChatState {
    pub(super) draft_model_id: Option<String>,
    pub(super) selected_request_id: Option<String>,
    pub(super) expanded_error_ids: HashSet<String>,
    pub(super) expanded_thinking_ids: HashSet<String>,
    pub(super) message_editor: Option<MessageEditor>,
    pub(crate) message_scroll: ScrollHandle,
    pub(crate) follow_latest: bool,
    pub(crate) system_prompt_mode: SystemPromptMode,
    pub(crate) system_prompt_editor: Option<Entity<Composer>>,
    pub(crate) generation_config_editor: Option<GenerationConfigEditor>,
    pub(crate) parameter_error: Option<String>,
    pub(crate) composer: Entity<Composer>,
    pub(super) generations: GenerationManager,
    pub(super) markdown_documents: HashMap<String, CachedMarkdown>,
}

pub(crate) struct SettingsState {
    pub(crate) section: SettingsSection,
    pub(crate) default_model_menu_open: bool,
    pub(crate) default_system_prompt_editor: Option<Entity<Composer>>,
    pub(crate) connection_tests: BTreeMap<String, ConnectionTestStatus>,
    pub(crate) provider_editor: Option<ProviderEditor>,
    pub(crate) model_editor: Option<ModelEditor>,
    pub(super) model_fetch_revision: u64,
    pub(crate) form_error: Option<String>,
}
