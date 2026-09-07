use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
    time::Instant,
};

use gpui::{Context, Entity, Task, Window, prelude::*};
use gpui_component::{
    input::{InputEvent, TextareaState},
    list::{ListEvent, ListState},
    select::{SelectEvent, SelectState},
    slider::{SliderEvent, SliderState},
};
use tokio::runtime::Runtime;

use super::{
    ChatState, ComposerImeHandler, DataState, DrawerMotion, FontRole, McpState, NavigationState,
    OneChat, OverlayState, Page, PlaybackState, Services, SettingsState, SidebarState,
    SidebarWidthMotion, TranslationState, TtsState, VisibilityMotion,
};
use crate::{
    desktop::{
        audio_playback::AudioPlayback,
        audio_recording::AudioRecording,
        ui::{
            SIDEBAR_WIDTH,
            inspector::InspectorTab,
            settings::{
                DefaultModelItem, FontFamilyItem, PromptSelectItem, ReasoningPresetSelectItem,
                SearchableItems, SettingsSection, ThemeColorControl, TitleModelItem,
            },
            shell::{
                CommandPaletteDelegate, ConversationSearchDelegate, ModelPickerDelegate,
                PromptPickerDelegate, ReasoningPickerDelegate,
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
        let mut this = Self::build(storage, runtime, mcp, window, cx);
        this.load_startup_snapshot(cx);
        this.reload_mcp(cx);
        this.start_audio_playback_observer(cx);
        this.start_audio_recording_observer(cx);
        this
    }

    pub(super) fn build(
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
            composer,
            composer_ime,
            mcp_json_import,
        } = input_controls(window, cx);
        let PickerControls {
            command_picker,
            conversation_search,
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
        let mcp_snapshot = McpSnapshot::empty(mcp.config_path());
        Self {
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
                hovered_conversation_id: None,
                generation_border_epoch: Instant::now(),
                unseen_generations: HashMap::new(),
                #[cfg(target_os = "macos")]
                conversation_peek: Default::default(),
                #[cfg(target_os = "macos")]
                new_conversation_force_click: Default::default(),
                #[cfg(target_os = "macos")]
                force_created_temporary_conversation: false,
                rename_editor: None,
                #[cfg(target_os = "macos")]
                rename_force_click: Default::default(),
                #[cfg(target_os = "macos")]
                force_renamed_conversation_id: None,
            },
            overlays: OverlayState {
                command_picker,
                conversation_search,
                model_picker,
                prompt_picker,
                reasoning_picker,
                active: None,
                motion: VisibilityMotion::new(false),
                previous_focus: None,
                response_model_turn_id: None,
                destructive_action: None,
            },
            playback: PlaybackState::new(cx),
            chat: ChatState::new(
                composer,
                composer_ime,
                conversation_history_limit_slider,
                timeline_focus,
            ),
            translation: TranslationState::new(window, cx),
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
                controls_dirty: true,
                prompt_preset_workspace: None,
                pending_prompt_preset_exit: None,
                prompt_variable_editor: None,
                prompt_variable_test_revision: 0,
                prompt_builtins_expanded: false,
                title_prompt_editor: None,
                translation_system_prompt_editor: None,
                translation_user_prompt_editor: None,
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
        }
    }
}
