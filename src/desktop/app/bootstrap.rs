use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
    time::Instant,
};

use gpui::{Context, Entity, ScrollHandle, Task, Window, prelude::*};
use gpui_component::{
    WindowExt as _,
    input::{InputEvent, InputState, TextareaState},
    list::{ListEvent, ListState},
    select::{SelectEvent, SelectState},
    slider::{SliderEvent, SliderState},
};
use tokio::runtime::Runtime;

use super::{
    ChatState, DataState, DefaultModelRole, DrawerMotion, FontRole, McpState, MessageScrollMotion,
    NavigationState, OneChat, OverlayState, Page, PlaybackState, Services, SettingsState,
    SidebarState, SidebarWidthMotion, SystemPromptMode, TimelineState, TtsState, VisibilityMotion,
};
use crate::{
    application::generation::GenerationManager,
    desktop::{
        audio_playback::AudioPlayback,
        audio_recording::{AudioRecording, RecordingSnapshot},
        ui::{
            SIDEBAR_WIDTH,
            inspector::InspectorTab,
            selectable_text::TextSelection,
            settings::{
                DefaultModelItem, FontFamilyItem, PromptSelectItem, ReasoningPresetSelectItem,
                SearchableItems, SettingsSection, ThemeColorControl,
            },
            shell::{
                CommandPaletteDelegate, ModelPickerDelegate, PromptPickerDelegate,
                ReasoningPickerDelegate,
            },
        },
    },
    domain::AppSettings,
    mcp::{McpManager, McpSnapshot},
    storage::{Storage, StorageSnapshot},
};

mod controls;

use controls::*;

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
        let applied_component_theme = None;
        let InputControls {
            search_input,
            composer,
            mcp_json_import,
        } = input_controls(window, cx);
        let PickerControls {
            command_picker,
            model_picker,
            prompt_picker,
            reasoning_picker,
        } = picker_controls(window, cx);
        let SliderControls {
            theme_color,
            message_width_slider,
            message_font_size_slider,
            background_opacity_slider,
            history_limit_slider,
            conversation_history_limit_slider,
        } = slider_controls(window, cx);
        let SelectControls {
            primary_model_select,
            title_model_select,
            title_reasoning_select,
            default_prompt_select,
            ui_font_select,
            code_font_select,
        } = select_controls(window, cx);
        let text_selection = TextSelection::new(cx.focus_handle());
        let mcp_snapshot = McpSnapshot::empty(mcp.config_path());
        let mut this = Self {
            root_focus,
            services: Services {
                storage,
                runtime,
                mcp,
                audio_playback: AudioPlayback::new(),
                audio_recording: AudioRecording::new(),
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
                sidebar_width_motion: SidebarWidthMotion::new(SIDEBAR_WIDTH),
                inspector_motion: DrawerMotion::new(false),
            },
            sidebar: SidebarState {
                width: SIDEBAR_WIDTH,
                search_input,
                hovered_conversation_id: None,
                generation_border_epoch: Instant::now(),
                unseen_generations: HashMap::new(),
                conversation_peek: Default::default(),
                rename_editor: None,
                #[cfg(target_os = "macos")]
                rename_force_click: Default::default(),
                #[cfg(target_os = "macos")]
                force_renamed_conversation_id: None,
            },
            overlays: OverlayState {
                command_picker,
                model_picker,
                prompt_picker,
                reasoning_picker,
                picker: None,
                picker_motion: VisibilityMotion::new(false),
                picker_previous_focus: None,
                response_model_turn_id: None,
                destructive_action: None,
            },
            playback: PlaybackState::new(cx),
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
                branch_swipe: Default::default(),
                response_tab_force_click: Default::default(),
                horizontal_scrolls: Default::default(),
                thinking_scrolls: HashMap::new(),
                thinking_motions: HashMap::new(),
                thinking_started_at: HashMap::new(),
                follow_latest: true,
                system_prompt_mode: SystemPromptMode::default(),
                system_prompt_editor: None,
                generation_config_editor: None,
                history_limit_slider: conversation_history_limit_slider,
                history_limit_preview: None,
                generation_config_save_revision: 0,
                parameter_error: None,
                composer,
                composer_multiline: Cell::new(false),
                composer_expanded: Cell::new(false),
                context_usage_popover_open: false,
                context_usage_popover_motion: VisibilityMotion::new(false),
                attachments: Vec::new(),
                attachment_previews: HashMap::new(),
                attachments_loading: false,
                attachments_revision: 0,
                audio_recording: RecordingSnapshot::default(),
                audio_recording_task: Task::ready(()),
                recording_conversation_id: None,
                generations: GenerationManager::default(),
                markdown_documents: HashMap::new(),
                pending_title_transitions: HashMap::new(),
                title_transitions: HashMap::new(),
            },
            tts: TtsState::new(window, cx),
            settings_ui: SettingsState {
                section: SettingsSection::default(),
                ui_font_select,
                code_font_select,
                theme_color,
                theme_color_save_revision: 0,
                message_font_size_slider,
                background_opacity_slider,
                message_width_slider,
                history_limit_slider,
                history_limit_save_pending: false,
                primary_model_select,
                title_model_select,
                title_reasoning_select,
                default_prompt_select,
                synced_primary_models: Vec::new(),
                synced_title_models: Vec::new(),
                synced_title_reasoning_presets: Vec::new(),
                synced_prompts: Vec::new(),
                viewed_prompt_preset: None,
                prompt_preset_editor: None,
                prompt_variable_editor: None,
                prompt_variable_test_revision: 0,
                prompt_builtins_expanded: false,
                title_prompt_editor: None,
                mcp_json_import,
                mcp_server_editor: None,
                mcp_error: None,
                expanded_mcp_server_ids: HashSet::new(),
                mcp_connection_tests: BTreeMap::new(),
                connection_tests: BTreeMap::new(),
                provider_drop_target: None,
                provider_editor: None,
                pending_provider_exit: None,
                model_editor: None,
                model_fetch_revision: 0,
                form_error: None,
            },
            applied_component_theme,
        };
        this.load_startup_snapshot(cx);
        this.reload_mcp(cx);
        this.start_audio_playback_observer(cx);
        this.start_audio_recording_observer(cx);
        this
    }
}
