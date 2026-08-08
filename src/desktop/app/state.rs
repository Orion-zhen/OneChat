use super::*;

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

const DRAWER_RESPONSE_SECONDS: f32 = 0.34;
const DRAWER_DAMPING_RATIO: f32 = 1.0;

pub(crate) struct DrawerMotion {
    value: f32,
    velocity: f32,
    target: f32,
    last_frame: Option<Instant>,
}

const VISIBILITY_MOTION_DURATION: Duration = Duration::from_millis(180);

pub(crate) struct VisibilityMotion {
    value: f32,
    from: f32,
    target: f32,
    started_at: Option<Instant>,
}

impl VisibilityMotion {
    pub(crate) fn new(visible: bool) -> Self {
        let value = f32::from(visible);
        Self {
            value,
            from: value,
            target: value,
            started_at: None,
        }
    }

    pub(crate) fn set_visible(&mut self, visible: bool) {
        self.set_visible_at(visible, Instant::now());
    }

    fn set_visible_at(&mut self, visible: bool, now: Instant) {
        self.advance(now);
        let target = f32::from(visible);
        if (target - self.target).abs() < f32::EPSILON {
            return;
        }

        self.from = self.value;
        self.target = target;
        self.started_at = Some(now);
    }

    pub(crate) fn progress(&mut self, window: &mut Window) -> f32 {
        self.advance(Instant::now());
        if self.started_at.is_some() {
            window.request_animation_frame();
        }
        self.value
    }

    fn snap(&mut self) {
        self.value = self.target;
        self.from = self.target;
        self.started_at = None;
    }

    fn advance(&mut self, now: Instant) {
        let Some(started_at) = self.started_at else {
            return;
        };
        let delta = (now - started_at).as_secs_f32() / VISIBILITY_MOTION_DURATION.as_secs_f32();
        if delta >= 1.0 {
            self.snap();
            return;
        }

        let eased = gpui::ease_out_quint()(delta);
        self.value = self.from + (self.target - self.from) * eased;
    }
}

impl DrawerMotion {
    pub(crate) fn new(open: bool) -> Self {
        let value = f32::from(open);
        Self {
            value,
            velocity: 0.0,
            target: value,
            last_frame: None,
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

        self.target = target;
        self.last_frame = Some(now);
    }

    pub(crate) fn snap(&mut self, open: bool) {
        let value = f32::from(open);
        self.value = value;
        self.velocity = 0.0;
        self.target = value;
        self.last_frame = None;
    }

    pub(crate) fn progress(&mut self, window: &mut Window, reduce_motion: bool) -> f32 {
        if reduce_motion {
            self.snap(self.target > 0.5);
            return self.value;
        }
        self.advance(Instant::now());
        if self.last_frame.is_some() {
            window.request_animation_frame();
        }
        self.value
    }

    fn advance(&mut self, now: Instant) {
        let Some(last_frame) = self.last_frame else {
            return;
        };

        let elapsed = (now - last_frame).as_secs_f32().min(0.064);
        self.last_frame = Some(now);
        self.step(elapsed);
    }

    fn step(&mut self, elapsed: f32) {
        let steps = (elapsed / (1.0 / 120.0)).ceil().max(1.0) as usize;
        let delta = elapsed / steps as f32;
        let omega = std::f32::consts::TAU / DRAWER_RESPONSE_SECONDS;
        for _ in 0..steps {
            let acceleration = omega * omega * (self.target - self.value)
                - 2.0 * DRAWER_DAMPING_RATIO * omega * self.velocity;
            self.velocity += acceleration * delta;
            self.value += self.velocity * delta;
        }

        self.value = self.value.clamp(0.0, 1.0);
        if (self.target - self.value).abs() < 0.001 && self.velocity.abs() < 0.001 {
            self.value = self.target;
            self.velocity = 0.0;
            self.last_frame = None;
        }
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
    pub(crate) response_model_turn_id: Option<String>,
    pub(crate) destructive_action: Option<DestructiveAction>,
}

#[derive(Clone, Copy)]
pub(crate) struct ThinkingMotion {
    pub(crate) from_height: f32,
    pub(crate) full_height: f32,
}

const MESSAGE_SCROLL_DURATION: Duration = Duration::from_millis(250);

pub(crate) struct MessageScrollMotion {
    from: f32,
    started_at: Option<Instant>,
}

impl MessageScrollMotion {
    pub(crate) fn new() -> Self {
        Self {
            from: 0.0,
            started_at: None,
        }
    }

    pub(crate) fn start(&mut self, from: f32) {
        self.from = from;
        self.started_at = Some(Instant::now());
    }

    pub(crate) fn cancel(&mut self) {
        self.started_at = None;
    }

    pub(crate) fn is_active(&self) -> bool {
        self.started_at.is_some()
    }

    pub(crate) fn offset(&mut self, target: f32, window: &mut Window) -> Option<(f32, bool)> {
        let started_at = self.started_at?;
        let delta = started_at.elapsed().as_secs_f32() / MESSAGE_SCROLL_DURATION.as_secs_f32();
        if delta >= 1.0 {
            self.started_at = None;
            return Some((target, true));
        }

        window.request_animation_frame();
        let progress = strong_ease_in_out(delta);
        Some((self.from + (target - self.from) * progress, false))
    }
}

fn strong_ease_in_out(delta: f32) -> f32 {
    let target_x = delta.clamp(0.0, 1.0);
    if target_x == 0.0 || target_x == 1.0 {
        return target_x;
    }

    let mut lower = 0.0;
    let mut upper = 1.0;
    for _ in 0..16 {
        let time = (lower + upper) / 2.0;
        if cubic_bezier_coordinate(time, 0.77, 0.175) < target_x {
            lower = time;
        } else {
            upper = time;
        }
    }
    cubic_bezier_coordinate((lower + upper) / 2.0, 0.0, 1.0)
}

fn cubic_bezier_coordinate(time: f32, control_1: f32, control_2: f32) -> f32 {
    let inverse = 1.0 - time;
    3.0 * inverse * inverse * time * control_1
        + 3.0 * inverse * time * time * control_2
        + time * time * time
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
