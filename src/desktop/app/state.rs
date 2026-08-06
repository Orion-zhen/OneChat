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
    pub(crate) inspector_motion: DrawerMotion,
}

const DRAWER_DURATION: Duration = Duration::from_millis(220);

pub(crate) struct DrawerMotion {
    value: f32,
    from: f32,
    target: f32,
    started_at: Option<Instant>,
    duration: Duration,
}

impl DrawerMotion {
    pub(crate) fn new(open: bool) -> Self {
        let value = f32::from(open);
        Self {
            value,
            from: value,
            target: value,
            started_at: None,
            duration: Duration::ZERO,
        }
    }

    pub(crate) fn set_open(&mut self, open: bool, animated: bool) {
        let now = Instant::now();
        self.advance(now);
        let target = f32::from(open);
        if (target - self.target).abs() < f32::EPSILON {
            return;
        }
        if !animated {
            self.snap(open);
            return;
        }

        self.from = self.value;
        self.target = target;
        self.duration = DRAWER_DURATION.mul_f32((target - self.value).abs());
        self.started_at = Some(now);
    }

    pub(crate) fn snap(&mut self, open: bool) {
        let value = f32::from(open);
        self.value = value;
        self.from = value;
        self.target = value;
        self.started_at = None;
        self.duration = Duration::ZERO;
    }

    pub(crate) fn progress(&mut self, window: &mut Window) -> f32 {
        self.advance(Instant::now());
        if self.started_at.is_some() {
            window.request_animation_frame();
        }
        self.value
    }

    fn advance(&mut self, now: Instant) {
        let Some(started_at) = self.started_at else {
            return;
        };
        let delta = (now - started_at).as_secs_f32() / self.duration.as_secs_f32();
        if delta >= 1.0 {
            self.value = self.target;
            self.started_at = None;
            return;
        }
        let eased = ease_out_quint()(delta);
        self.value = self.from + (self.target - self.from) * eased;
    }
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

#[derive(Clone, Copy)]
pub(crate) struct ThinkingMotion {
    pub(crate) from_height: f32,
    pub(crate) full_height: f32,
}

pub(crate) struct ChatState {
    pub(super) draft_model_id: Option<String>,
    pub(super) selected_request_id: Option<String>,
    pub(super) expanded_error_ids: HashSet<String>,
    pub(super) collapsed_thinking_ids: HashSet<String>,
    pub(super) message_editor: Option<MessageEditor>,
    pub(crate) message_scroll: ScrollHandle,
    pub(crate) thinking_scrolls: HashMap<String, ScrollHandle>,
    pub(crate) thinking_motions: HashMap<String, ThinkingMotion>,
    pub(crate) thinking_started_at: HashMap<String, Instant>,
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
