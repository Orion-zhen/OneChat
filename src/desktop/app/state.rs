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
    pub(crate) hovered_conversation_id: Option<String>,
    pub(super) rename_editor: Option<RenameEditor>,
}

pub(crate) struct OverlayState {
    pub(crate) command_palette_open: bool,
    pub(crate) command_query: String,
    pub(crate) command_input: Entity<Composer>,
    pub(crate) command_selection: usize,
    pub(crate) command_scroll: ScrollHandle,
    pub(crate) model_picker_open: bool,
    pub(crate) response_model_turn_id: Option<String>,
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
    pub(super) message_editor: Option<MessageEditor>,
    pub(crate) message_scroll: ScrollHandle,
    pub(crate) message_scroll_motion: MessageScrollMotion,
    pub(crate) text_selection: TextSelection,
    pub(crate) thinking_scrolls: HashMap<String, ScrollHandle>,
    pub(crate) thinking_motions: HashMap<String, ThinkingMotion>,
    pub(crate) thinking_started_at: HashMap<String, Instant>,
    pub(crate) follow_latest: bool,
    pub(crate) system_prompt_mode: SystemPromptMode,
    pub(crate) system_prompt_editor: Option<Entity<Composer>>,
    pub(crate) generation_config_editor: Option<GenerationConfigEditor>,
    pub(crate) generation_config_save_revision: u64,
    pub(crate) parameter_error: Option<String>,
    pub(crate) composer: Entity<Composer>,
    pub(super) generations: GenerationManager,
    pub(super) markdown_documents: HashMap<String, CachedMarkdown>,
}

pub(crate) struct SettingsState {
    pub(crate) section: SettingsSection,
    pub(crate) message_width_dragging: bool,
    pub(crate) default_model_menu_open: bool,
    pub(crate) default_system_prompt_editor: Option<Entity<Composer>>,
    pub(crate) connection_tests: BTreeMap<String, ConnectionTestStatus>,
    pub(crate) provider_editor: Option<ProviderEditor>,
    pub(crate) model_editor: Option<ModelEditor>,
    pub(super) model_fetch_revision: u64,
    pub(crate) form_error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::strong_ease_in_out;

    #[test]
    fn message_scroll_easing_is_bounded_and_monotonic() {
        let mut previous = strong_ease_in_out(0.0);
        assert_eq!(previous, 0.0);

        for step in 1..=100 {
            let current = strong_ease_in_out(step as f32 / 100.0);
            assert!((0.0..=1.0).contains(&current));
            assert!(current >= previous);
            previous = current;
        }

        assert_eq!(previous, 1.0);
    }
}
