use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use gpui::{Entity, FocusHandle, ScrollHandle, Task};
use gpui_component::{input::TextareaState, slider::SliderState};

use super::super::{
    ComposerImeHandler, MessageEditor, MessageScrollMotion, PendingTitleTransition,
    ResponsePresentation, SearchTarget, SystemPromptMode, TimelineState, TitleTransition,
    VisibilityMotion,
};
use crate::{
    application::generation::GenerationManager,
    desktop::{
        audio_recording::RecordingSnapshot,
        branch_swipe::{BranchSwipeState, BranchSwipeTarget},
        ui::inspector::GenerationConfigEditor,
    },
    domain::AttachmentDraft,
};

pub(crate) struct ChatState {
    pub(in crate::desktop::app) draft_model_id: Option<String>,
    pub(in crate::desktop::app) transient_conversation_id: Option<String>,
    pub(in crate::desktop::app) selected_request_id: Option<String>,
    pub(crate) visible_response_ids: HashMap<String, String>,
    pub(in crate::desktop::app) pending_search_target: Option<SearchTarget>,
    pub(crate) search_highlight_id: Option<String>,
    pub(in crate::desktop::app) expanded_error_ids: HashSet<String>,
    pub(in crate::desktop::app) expanded_tool_execution_ids: HashSet<String>,
    pub(crate) expanded_conversation_tool_server_ids: HashSet<String>,
    pub(in crate::desktop::app) message_editor: Option<MessageEditor>,
    pub(crate) message_scroll: ScrollHandle,
    pub(crate) message_scroll_motion: MessageScrollMotion,
    pub(crate) jump_to_latest_motion: VisibilityMotion,
    pub(crate) timeline: TimelineState,
    pub(crate) presentation: ResponsePresentation,
    pub(crate) branch_swipe: BranchSwipeState<BranchSwipeTarget>,
    #[cfg(target_os = "macos")]
    pub(crate) response_tab_force_click: crate::desktop::pressure_touch::ForceClickGesture<String>,
    pub(crate) follow_latest: bool,
    pub(crate) system_prompt_mode: SystemPromptMode,
    pub(crate) system_prompt_editor: Option<Entity<TextareaState>>,
    pub(crate) assistant_opening_editor: Option<Entity<TextareaState>>,
    pub(crate) generation_config_editor: Option<GenerationConfigEditor>,
    pub(crate) controls_dirty: bool,
    pub(crate) history_limit_slider: Entity<SliderState>,
    pub(crate) history_limit_preview: Option<crate::domain::HistoryLimit>,
    pub(crate) generation_config_save_revision: u64,
    pub(crate) parameter_error: Option<String>,
    pub(crate) composer: Entity<TextareaState>,
    pub(crate) composer_ime: Entity<ComposerImeHandler>,
    pub(crate) composer_committed_value: String,
    pub(crate) composer_multiline: Cell<bool>,
    pub(crate) composer_expanded: Cell<bool>,
    pub(crate) context_usage_popover_open: bool,
    pub(crate) context_usage_popover_motion: VisibilityMotion,
    pub(crate) attachments: Vec<AttachmentDraft>,
    pub(crate) attachment_previews: HashMap<String, Arc<gpui::Image>>,
    pub(crate) temporary_attachment_files: HashMap<String, Vec<u8>>,
    pub(crate) attachments_loading: bool,
    pub(crate) attachments_revision: u64,
    pub(crate) audio_recording: RecordingSnapshot,
    pub(in crate::desktop::app) audio_recording_task: Task<()>,
    pub(in crate::desktop::app) recording_conversation_id: Option<String>,
    pub(in crate::desktop::app) generations: GenerationManager,
    pub(in crate::desktop::app) pending_title_transitions: HashMap<String, PendingTitleTransition>,
    pub(in crate::desktop::app) title_transitions: HashMap<String, TitleTransition>,
}

impl ChatState {
    pub(in crate::desktop::app) fn new(
        composer: Entity<TextareaState>,
        composer_ime: Entity<ComposerImeHandler>,
        history_limit_slider: Entity<SliderState>,
        timeline_focus: FocusHandle,
    ) -> Self {
        Self {
            draft_model_id: None,
            transient_conversation_id: None,
            selected_request_id: None,
            visible_response_ids: HashMap::new(),
            pending_search_target: None,
            search_highlight_id: None,
            expanded_error_ids: HashSet::new(),
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
            presentation: Default::default(),
            branch_swipe: Default::default(),
            #[cfg(target_os = "macos")]
            response_tab_force_click: Default::default(),
            follow_latest: true,
            system_prompt_mode: SystemPromptMode::default(),
            system_prompt_editor: None,
            assistant_opening_editor: None,
            generation_config_editor: None,
            controls_dirty: true,
            history_limit_slider,
            history_limit_preview: None,
            generation_config_save_revision: 0,
            parameter_error: None,
            composer,
            composer_ime,
            composer_committed_value: String::new(),
            composer_multiline: Cell::new(false),
            composer_expanded: Cell::new(false),
            context_usage_popover_open: false,
            context_usage_popover_motion: VisibilityMotion::new(false),
            attachments: Vec::new(),
            attachment_previews: HashMap::new(),
            temporary_attachment_files: HashMap::new(),
            attachments_loading: false,
            attachments_revision: 0,
            audio_recording: RecordingSnapshot::default(),
            audio_recording_task: Task::ready(()),
            recording_conversation_id: None,
            generations: GenerationManager::default(),
            pending_title_transitions: HashMap::new(),
            title_transitions: HashMap::new(),
        }
    }

    pub(in crate::desktop::app) fn reset_conversation(&mut self) {
        let mut next = Self::new(
            self.composer.clone(),
            self.composer_ime.clone(),
            self.history_limit_slider.clone(),
            self.timeline.focus.clone(),
        );
        next.transient_conversation_id = self.transient_conversation_id.take();
        next.pending_search_target = self.pending_search_target.take();
        next.search_highlight_id = self.search_highlight_id.take();
        next.composer_committed_value = std::mem::take(&mut self.composer_committed_value);
        next.composer_multiline.set(self.composer_multiline.get());
        next.composer_expanded.set(self.composer_expanded.get());
        next.generations = std::mem::take(&mut self.generations);
        next.pending_title_transitions = std::mem::take(&mut self.pending_title_transitions);
        next.title_transitions = std::mem::take(&mut self.title_transitions);
        next.presentation.thinking_started_at =
            std::mem::take(&mut self.presentation.thinking_started_at);
        next.audio_recording = self.audio_recording.clone();
        next.audio_recording_task =
            std::mem::replace(&mut self.audio_recording_task, Task::ready(()));
        next.recording_conversation_id = self.recording_conversation_id.take();
        next.generation_config_save_revision = self.generation_config_save_revision.wrapping_add(1);
        next.attachments_revision = self.attachments_revision.wrapping_add(1);
        next.message_scroll.scroll_to_bottom();
        *self = next;
    }
}
