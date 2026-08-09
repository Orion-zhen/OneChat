use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

use gpui::{Context, ScrollHandle, Task, Window, prelude::*};
use gpui_component::{
    WindowExt as _,
    input::{InputEvent, InputState},
    list::{ListEvent, ListState},
    select::{SelectEvent, SelectState},
    slider::{SliderEvent, SliderState},
};
use tokio::runtime::Runtime;

use super::{
    ChatState, DataState, DefaultModelRole, DrawerMotion, FontRole, InspectorPointerState,
    McpState, MessageScrollMotion, NavigationState, OneChat, OverlayState, Page, Services,
    SettingsState, SidebarState, SystemPromptMode, TimelineState, VisibilityMotion,
};
use crate::{
    application::generation::GenerationManager,
    desktop::ui::{
        inspector::InspectorTab,
        selectable_text::TextSelection,
        settings::{
            DefaultModelItem, FontFamilyItem, PromptSelectItem, ReasoningPresetSelectItem,
            SearchableItems, SettingsSection,
        },
        shell::{
            CommandPaletteDelegate, ModelPickerDelegate, PromptPickerDelegate,
            ReasoningPickerDelegate,
        },
    },
    domain::{AppSettings, Theme},
    mcp::{McpManager, McpSnapshot},
    storage::{Storage, StorageSnapshot},
};

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
                .placeholder("Message")
        });
        cx.subscribe_in(&composer, window, |_, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
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

        let reasoning_picker = cx.new(|cx| {
            ListState::new(ReasoningPickerDelegate::empty(), window, cx).searchable(true)
        });
        cx.subscribe_in(
            &reasoning_picker,
            window,
            |this, picker, event: &ListEvent, window, cx| {
                let ListEvent::Confirm(index) = event else {
                    return;
                };
                let preset = picker.read(cx).delegate().selected_id(*index);
                if let Some(preset) = preset {
                    window.close_dialog(cx);
                    this.select_reasoning_preset(Some(preset), cx);
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

        let title_reasoning_select = cx
            .new(|cx| SelectState::new(Vec::<ReasoningPresetSelectItem>::new(), None, window, cx));
        cx.subscribe(
            &title_reasoning_select,
            |this, _, event: &SelectEvent<Vec<ReasoningPresetSelectItem>>, cx| {
                let SelectEvent::Confirm(value) = event;
                if let Some(value) = value.clone() {
                    this.select_title_generation_reasoning_preset(value, cx);
                }
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
                reasoning_picker,
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
                composer_multiline: Cell::new(false),
                composer_expanded: Cell::new(false),
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
                title_reasoning_select,
                default_prompt_select,
                synced_primary_models: Vec::new(),
                synced_title_models: Vec::new(),
                synced_title_reasoning_presets: Vec::new(),
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
}
