use super::*;

pub(super) struct InputControls {
    pub(super) composer: Entity<TextareaState>,
    pub(super) composer_ime: Entity<ComposerImeHandler>,
    pub(super) mcp_json_import: Entity<TextareaState>,
}

pub(super) fn input_controls(window: &mut Window, cx: &mut Context<OneChat>) -> InputControls {
    let composer = cx.new(|cx| {
        TextareaState::new(window, cx)
            .auto_grow(1, 8)
            .soft_wrap(true)
            .placeholder("Message")
    });
    let composer_ime = cx.new(|_| ComposerImeHandler::new(composer.clone()));
    cx.subscribe_in(
        &composer,
        window,
        |this, composer, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.chat.composer_committed_value = composer.read(cx).value().to_string();
                cx.notify();
            }
        },
    )
    .detach();

    let mcp_json_import = cx.new(|cx| {
        TextareaState::new(window, cx)
            .soft_wrap(true)
            .placeholder("Paste a JSON or JSONC object containing mcpServers")
    });
    InputControls {
        composer,
        composer_ime,
        mcp_json_import,
    }
}

pub(super) struct PickerControls {
    pub(super) command_picker: Entity<ListState<CommandPaletteDelegate>>,
    pub(super) conversation_search: Entity<ListState<ConversationSearchDelegate>>,
    pub(super) model_picker: Entity<ListState<ModelPickerDelegate>>,
    pub(super) prompt_picker: Entity<ListState<PromptPickerDelegate>>,
    pub(super) reasoning_picker: Entity<ListState<ReasoningPickerDelegate>>,
}

pub(super) fn picker_controls(window: &mut Window, cx: &mut Context<OneChat>) -> PickerControls {
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
                this.close_shell_overlay_immediate(window, cx);
                this.execute_command(command, window, cx);
            }
        },
    )
    .detach();

    let conversation_search = cx
        .new(|cx| ListState::new(ConversationSearchDelegate::empty(), window, cx).searchable(true));
    cx.subscribe_in(
        &conversation_search,
        window,
        |this, search, event: &ListEvent, _, cx| {
            let ListEvent::Confirm(index) = event else {
                return;
            };
            let result = search.read(cx).delegate().result(*index);
            if let Some(result) = result {
                this.open_conversation_search_result(result, cx);
            }
        },
    )
    .detach();

    let model_picker =
        cx.new(|cx| ListState::new(ModelPickerDelegate::empty(), window, cx).searchable(true));
    cx.subscribe_in(
        &model_picker,
        window,
        |this, picker, event: &ListEvent, _, cx| {
            let ListEvent::Confirm(index) = event else {
                return;
            };
            let model_id = picker.read(cx).delegate().selected_model_id(*index);
            if let Some(model_id) = model_id {
                this.select_model(model_id, cx);
                this.close_shell_overlay(true, cx);
            }
        },
    )
    .detach();

    let prompt_picker =
        cx.new(|cx| ListState::new(PromptPickerDelegate::empty(), window, cx).searchable(true));
    cx.subscribe_in(
        &prompt_picker,
        window,
        |this, picker, event: &ListEvent, _, cx| {
            let ListEvent::Confirm(index) = event else {
                return;
            };
            let name = picker.read(cx).delegate().selected_name(*index);
            if let Some(name) = name {
                this.select_prompt_preset(name, cx);
                this.close_shell_overlay(true, cx);
            }
        },
    )
    .detach();

    let reasoning_picker =
        cx.new(|cx| ListState::new(ReasoningPickerDelegate::empty(), window, cx).searchable(true));
    cx.subscribe_in(
        &reasoning_picker,
        window,
        |this, picker, event: &ListEvent, _, cx| {
            let ListEvent::Confirm(index) = event else {
                return;
            };
            let preset = picker.read(cx).delegate().selected_id(*index);
            if let Some(preset) = preset {
                this.select_reasoning_preset(Some(preset), cx);
                this.close_shell_overlay(true, cx);
            }
        },
    )
    .detach();

    PickerControls {
        command_picker,
        conversation_search,
        model_picker,
        prompt_picker,
        reasoning_picker,
    }
}

pub(super) struct SliderControls {
    pub(super) theme_color: ThemeColorControl,
    pub(super) message_width_slider: Entity<SliderState>,
    pub(super) message_font_size_slider: Entity<SliderState>,
    pub(super) background_opacity_slider: Entity<SliderState>,
    pub(super) history_limit_slider: Entity<SliderState>,
    pub(super) conversation_history_limit_slider: Entity<SliderState>,
}

pub(super) fn slider_controls(window: &mut Window, cx: &mut Context<OneChat>) -> SliderControls {
    let theme_color = ThemeColorControl::new(window, cx);

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

    let history_limit_slider = cx.new(|_| {
        SliderState::new()
            .min(crate::domain::HISTORY_LIMIT_SLIDER_MIN)
            .max(crate::domain::HISTORY_LIMIT_SLIDER_MAX)
            .step(crate::domain::HISTORY_LIMIT_SLIDER_STEP)
            .default_value(AppSettings::default().history_limit.slider_value())
    });
    cx.subscribe(
        &history_limit_slider,
        |this, _, event: &SliderEvent, cx| match event {
            SliderEvent::Change(value) => {
                this.update_history_limit(value.start(), cx);
            }
            SliderEvent::Release(value) => {
                this.update_history_limit(value.start(), cx);
                this.save_history_limit_if_changed(cx);
            }
        },
    )
    .detach();

    let conversation_history_limit_slider = cx.new(|_| {
        SliderState::new()
            .min(crate::domain::HISTORY_LIMIT_SLIDER_MIN)
            .max(crate::domain::HISTORY_LIMIT_SLIDER_MAX)
            .step(crate::domain::HISTORY_LIMIT_SLIDER_STEP)
            .default_value(AppSettings::default().history_limit.slider_value())
    });
    cx.subscribe(
        &conversation_history_limit_slider,
        |this, _, event: &SliderEvent, cx| match event {
            SliderEvent::Change(value) => {
                this.preview_conversation_history_limit(value.start(), cx);
            }
            SliderEvent::Release(value) => {
                this.commit_conversation_history_limit(value.start(), cx);
            }
        },
    )
    .detach();

    SliderControls {
        theme_color,
        message_width_slider,
        message_font_size_slider,
        background_opacity_slider,
        history_limit_slider,
        conversation_history_limit_slider,
    }
}

pub(super) struct SelectControls {
    pub(super) primary_model_select: Entity<SelectState<Vec<DefaultModelItem>>>,
    pub(super) title_model_select: Entity<SelectState<Vec<TitleModelItem>>>,
    pub(super) title_reasoning_select: Entity<SelectState<Vec<ReasoningPresetSelectItem>>>,
    pub(super) default_prompt_select: Entity<SelectState<Vec<PromptSelectItem>>>,
    pub(super) ui_font_select: Entity<SelectState<SearchableItems<FontFamilyItem>>>,
    pub(super) code_font_select: Entity<SelectState<SearchableItems<FontFamilyItem>>>,
}

pub(super) fn select_controls(window: &mut Window, cx: &mut Context<OneChat>) -> SelectControls {
    let primary_model_select =
        cx.new(|cx| SelectState::new(Vec::<DefaultModelItem>::new(), None, window, cx));
    cx.subscribe(
        &primary_model_select,
        |this, _, event: &SelectEvent<Vec<DefaultModelItem>>, cx| {
            let SelectEvent::Confirm(value) = event;
            this.select_primary_model(value.clone().flatten(), cx);
        },
    )
    .detach();

    let title_model_select =
        cx.new(|cx| SelectState::new(Vec::<TitleModelItem>::new(), None, window, cx));
    cx.subscribe(
        &title_model_select,
        |this, _, event: &SelectEvent<Vec<TitleModelItem>>, cx| {
            let SelectEvent::Confirm(Some(source)) = event else {
                return;
            };
            this.select_title_generation_model(source.clone(), cx);
        },
    )
    .detach();

    let title_reasoning_select =
        cx.new(|cx| SelectState::new(Vec::<ReasoningPresetSelectItem>::new(), None, window, cx));
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

    SelectControls {
        primary_model_select,
        title_model_select,
        title_reasoning_select,
        default_prompt_select,
        ui_font_select,
        code_font_select,
    }
}
