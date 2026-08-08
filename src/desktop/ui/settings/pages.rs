use super::*;

pub(super) fn general_page(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let appearance = div()
        .w_full()
        .flex()
        .flex_col()
        .child(setting_row(
            "Theme",
            "Match the Mac or choose a fixed appearance.",
            theme_selector(app, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row_with_preview(
            "Interface Font",
            font_preview(FontRole::Ui, cx),
            font_stack_editor(app, FontRole::Ui, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row_with_preview(
            "Code Font",
            font_preview(FontRole::Code, cx),
            font_stack_editor(app, FontRole::Code, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row(
            "Message Size",
            "Conversation text size; code stays one pixel smaller.",
            message_font_size_slider(app, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row(
            "Background Opacity",
            "Adjust the main window transparency and glass effect.",
            background_opacity_slider(app, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row(
            "Message Width",
            "Maximum width as a share of the available chat area.",
            message_width_slider(app, cx),
            cx,
        ));
    let automation = setting_row(
        "Automatic Titles",
        "Generate a title after the first completed response.",
        auto_title_toggle(app, cx),
        cx,
    );

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "General",
                "Choose how OneChat looks and responds.",
                cx,
            ))
            .child(section("Appearance", None, appearance, cx))
            .child(section("Automation", None, automation, cx)),
    )
}

fn auto_title_toggle(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    Switch::new("automatic-titles-toggle")
        .small()
        .checked(app.settings().auto_title_enabled)
        .color(cx.theme().primary)
        .on_click(cx.listener(|this, _: &bool, _, cx| this.toggle_auto_title_enabled(cx)))
        .into_any_element()
}

fn theme_selector(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let selected = match app.settings().theme {
        Theme::System => 0,
        Theme::Light => 1,
        Theme::Dark => 2,
    };
    TabBar::new("theme-selector")
        .segmented()
        .large()
        .w(px(300.0))
        .selected_index(selected)
        .child(Tab::new().w(px(96.0)).label("System"))
        .child(Tab::new().w(px(96.0)).label("Light"))
        .child(Tab::new().w(px(96.0)).label("Dark"))
        .on_click(cx.listener(|this, index: &usize, _, cx| {
            let theme = [Theme::System, Theme::Light, Theme::Dark][*index];
            this.set_theme(theme, cx);
        }))
        .into_any_element()
}

fn font_stack_editor(app: &OneChat, role: FontRole, cx: &mut Context<OneChat>) -> AnyElement {
    let families = match role {
        FontRole::Ui => &app.settings().ui_font_families,
        FontRole::Code => &app.settings().code_font_families,
    };
    let select = match role {
        FontRole::Ui => &app.settings_ui.ui_font_select,
        FontRole::Code => &app.settings_ui.code_font_select,
    };
    let count = families.len();
    let list = div()
        .w_full()
        .rounded(px(11.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .overflow_hidden()
        .children(families.iter().enumerate().map(|(index, family)| {
            let move_up = icon_action(
                SharedString::from(format!("font-{role:?}-{index}-up")),
                AppIcon::ArrowUp,
                IconTone::Muted,
                "Move earlier",
                cx,
            )
            .disabled(index == 0)
            .on_click(
                cx.listener(move |this, _, _, cx| this.move_font_family(role, index, true, cx)),
            );
            let move_down = icon_action(
                SharedString::from(format!("font-{role:?}-{index}-down")),
                AppIcon::ArrowDown,
                IconTone::Muted,
                "Move later",
                cx,
            )
            .disabled(index + 1 == count)
            .on_click(
                cx.listener(move |this, _, _, cx| this.move_font_family(role, index, false, cx)),
            );
            let remove = icon_action(
                SharedString::from(format!("font-{role:?}-{index}-remove")),
                AppIcon::Trash,
                IconTone::Danger,
                "Remove font",
                cx,
            )
            .disabled(count == 1)
            .on_click(cx.listener(move |this, _, _, cx| this.remove_font_family(role, index, cx)));

            div()
                .min_h(px(46.0))
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .when(index + 1 < count, |row| {
                    row.border_b_1().border_color(cx.theme().border)
                })
                .child(
                    div()
                        .size(px(22.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.0))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .bg(if index == 0 {
                            cx.theme().accent
                        } else {
                            cx.theme().transparent
                        })
                        .text_color(if index == 0 {
                            cx.theme().primary
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child((index + 1).to_string()),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_sm()
                        .font_weight(if index == 0 {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .child(font_family_label(family)),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap_0p5()
                        .child(move_up)
                        .child(move_down)
                        .child(remove),
                )
        }));
    div()
        .w(px(340.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(list)
        .child(
            Select::new(select)
                .large()
                .h(px(40.0))
                .px(px(12.0))
                .rounded(px(10.0))
                .icon(Icon::new(IconName::Plus))
                .placeholder("Add font…")
                .search_placeholder("Search installed fonts…")
                .menu_max_h(px(300.0))
                .w_full(),
        )
        .into_any_element()
}

fn font_preview(role: FontRole, cx: &App) -> AnyElement {
    let (font, text) = match role {
        FontRole::Ui => (
            crate::desktop::ui::theme::ui_font(cx),
            "The quick brown fox · 中文字体预览",
        ),
        FontRole::Code => (
            crate::desktop::ui::theme::code_font(cx),
            "let fallback = \"中文\";",
        ),
    };
    div()
        .pt_3()
        .font(font)
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text)
        .into_any_element()
}

fn background_opacity_slider(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    percentage_slider(
        &app.settings_ui.background_opacity_slider,
        app.settings().background_opacity(),
        cx,
    )
}

fn message_width_slider(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    percentage_slider(
        &app.settings_ui.message_width_slider,
        app.settings().message_width_ratio(),
        cx,
    )
}

fn message_font_size_slider(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    div()
        .w(px(236.0))
        .flex_none()
        .flex()
        .items_center()
        .gap_3()
        .child(
            Slider::new(&app.settings_ui.message_font_size_slider)
                .w(px(180.0))
                .bg(cx.theme().primary),
        )
        .child(
            div()
                .w(px(42.0))
                .flex_none()
                .text_right()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(format!("{:.0} px", app.settings().message_font_size())),
        )
        .into_any_element()
}

fn percentage_slider(
    state: &Entity<SliderState>,
    value: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .w(px(236.0))
        .flex_none()
        .flex()
        .items_center()
        .gap_3()
        .child(Slider::new(state).w(px(180.0)).bg(cx.theme().primary))
        .child(
            div()
                .w(px(42.0))
                .flex_none()
                .text_right()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(format!("{:.0}%", value * 100.0)),
        )
        .into_any_element()
}

pub(super) fn default_models_page(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let content = div()
        .w_full()
        .flex()
        .flex_col()
        .child(setting_row(
            "Primary Model",
            "Used when creating a new conversation.",
            default_model_select(app, DefaultModelRole::Primary),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row(
            "Title Generation Model",
            "Used for automatic titles after the first response.",
            default_model_select(app, DefaultModelRole::TitleGeneration),
            cx,
        ));

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "Default Models",
                "Choose the models OneChat uses by default.",
                cx,
            ))
            .child(section(
                "Model Selection",
                Some("Only models that are ready to use appear here."),
                content,
                cx,
            )),
    )
}

fn default_model_select(app: &OneChat, role: DefaultModelRole) -> AnyElement {
    let state = match role {
        DefaultModelRole::Primary => &app.settings_ui.primary_model_select,
        DefaultModelRole::TitleGeneration => &app.settings_ui.title_model_select,
    };
    Select::new(state)
        .large()
        .h(px(40.0))
        .px(px(12.0))
        .rounded(px(10.0))
        .placeholder(match role {
            DefaultModelRole::Primary => "Choose a model",
            DefaultModelRole::TitleGeneration => "Use Primary Model",
        })
        .menu_max_h(px(320.0))
        .w(px(300.0))
        .empty(|_, cx| {
            div()
                .p_3()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("No available models configured")
        })
        .into_any_element()
}

pub(super) fn system_prompts_page(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let preset_count = app.data.snapshot.prompt_presets.len();
    let preset_count_label = format!(
        "{preset_count} {}",
        if preset_count == 1 {
            "preset"
        } else {
            "presets"
        }
    );

    let conversation_default = setting_row(
        "Default Prompt",
        "Copied into each new conversation.",
        default_prompt_select(app),
        cx,
    );

    let prompt_library_actions = div()
        .flex_none()
        .flex()
        .items_center()
        .gap_2()
        .child(status_pill(preset_count_label, false, cx))
        .child(
            icon_action(
                "reload-prompt-presets",
                AppIcon::Regenerate,
                IconTone::Muted,
                "Reload prompts",
                cx,
            )
            .on_click(cx.listener(|this, _, _, cx| this.reload_prompt_presets(cx))),
        )
        .child(
            primary_icon_action("add-prompt-preset", AppIcon::Plus, "Add prompt", cx).on_click(
                cx.listener(|this, _, window, cx| this.begin_add_prompt_preset(window, cx)),
            ),
        );

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "System Prompts",
                "Set the instructions used in conversations and automatic titles.",
                cx,
            ))
            .child(section(
                "New Conversations",
                Some("Choose the reusable prompt applied when a chat is created."),
                conversation_default,
                cx,
            ))
            .child(section(
                "Automatic Titles",
                Some("Instructions used after the first completed response."),
                title_prompt_content(app, cx),
                cx,
            ))
            .child(section_with_actions(
                "Prompt Library",
                Some("Reusable Markdown prompts for conversations."),
                Some(prompt_library_actions.into_any_element()),
                prompt_presets_content(app, cx),
                cx,
            )),
    )
}

fn status_pill(label: impl Into<SharedString>, accent: bool, cx: &App) -> AnyElement {
    div()
        .flex_none()
        .rounded_full()
        .bg(if accent {
            cx.theme().accent
        } else {
            cx.theme().muted
        })
        .px_2()
        .py_1()
        .text_size(px(10.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if accent {
            cx.theme().primary
        } else {
            cx.theme().muted_foreground
        })
        .child(label.into())
        .into_any_element()
}

fn stretching_column() -> Div {
    let mut column = div().flex().flex_col();
    column.style().align_items = Some(AlignItems::Stretch);
    column
}

fn default_prompt_select(app: &OneChat) -> AnyElement {
    Select::new(&app.settings_ui.default_prompt_select)
        .large()
        .h(px(40.0))
        .px(px(12.0))
        .rounded(px(10.0))
        .placeholder("No System Prompt")
        .menu_max_h(px(320.0))
        .w(px(300.0))
        .into_any_element()
}

fn prompt_presets_content(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    if app.data.snapshot.prompt_presets.is_empty() {
        return div()
            .w_full()
            .px_4()
            .py_6()
            .flex()
            .flex_col()
            .items_center()
            .text_center()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("No presets yet"),
            )
            .child(
                div()
                    .pt_1()
                    .text_size(px(12.0))
                    .text_color(cx.theme().muted_foreground)
                    .child("Create a reusable prompt to get started."),
            )
            .into_any_element();
    }

    let mut cards = div().w_full().flex().flex_col().gap_1();
    for preset in &app.data.snapshot.prompt_presets {
        let view_name = preset.name.clone();
        let edit_name = preset.name.clone();
        let delete_name = preset.name.clone();
        let default =
            app.settings().default_system_prompt_preset.as_deref() == Some(preset.name.as_str());
        cards = cards.child(
            div()
                .id(SharedString::from(format!(
                    "prompt-preset-card-{}",
                    preset.name
                )))
                .w_full()
                .min_h(px(88.0))
                .rounded_lg()
                .bg(cx.theme().transparent)
                .hover(|style| style.bg(cx.theme().list_hover))
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(preset.name.clone()),
                                )
                                .children(default.then(|| status_pill("Default", true, cx))),
                        )
                        .child(
                            div()
                                .flex_none()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    icon_action(
                                        SharedString::from(format!("view-prompt-{}", preset.name)),
                                        AppIcon::Eye,
                                        IconTone::Muted,
                                        "View prompt",
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            this.view_prompt_preset(view_name.clone(), window, cx)
                                        },
                                    )),
                                )
                                .child(
                                    icon_action(
                                        SharedString::from(format!("edit-prompt-{}", preset.name)),
                                        AppIcon::Pencil,
                                        IconTone::Muted,
                                        "Edit prompt",
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            this.begin_edit_prompt_preset(
                                                edit_name.clone(),
                                                window,
                                                cx,
                                            )
                                        },
                                    )),
                                )
                                .child(
                                    icon_action(
                                        SharedString::from(format!(
                                            "delete-prompt-{}",
                                            preset.name
                                        )),
                                        AppIcon::Trash,
                                        IconTone::Danger,
                                        "Delete prompt",
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            this.request_delete_prompt_preset(
                                                delete_name.clone(),
                                                window,
                                                cx,
                                            )
                                        },
                                    )),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .max_h(px(36.0))
                        .overflow_hidden()
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(prompt_preview(&preset.content)),
                ),
        );
    }
    cards.into_any_element()
}

fn prompt_preset_field(label: &'static str, input: &Entity<InputState>, multiline: bool) -> Field {
    Field::new().label(label).required(true).child(
        Input::new(input)
            .aria_label(label)
            .large()
            .rounded(px(12.0))
            .when(multiline, |input| input.h(px(240.0))),
    )
}

pub(super) fn prompt_preset_dialog_body(app: &OneChat, cx: &App) -> AnyElement {
    if let Some(editor) = &app.settings_ui.prompt_preset_editor {
        return stretching_column()
            .px_5()
            .pb_5()
            .gap_3()
            .child(
                Form::vertical()
                    .child(prompt_preset_field("Name", &editor.name, false))
                    .child(prompt_preset_field("Prompt", &editor.content, true)),
            )
            .children(app.settings_ui.form_error.as_deref().map(error_banner))
            .into_any_element();
    }

    let preset = app
        .settings_ui
        .viewed_prompt_preset
        .as_deref()
        .and_then(|name| app.prompt_preset(name))
        .expect("prompt preset dialog requires a viewed preset or editor");
    stretching_column()
        .px_5()
        .pb_5()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child("Name"),
                )
                .child(
                    div()
                        .rounded_lg()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().muted)
                        .px_3()
                        .py_2()
                        .text_sm()
                        .child(preset.name.clone()),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child("Prompt"),
                )
                .child(
                    div()
                        .id("viewed-prompt-content")
                        .min_h(px(200.0))
                        .max_h(px(300.0))
                        .overflow_y_scroll()
                        .rounded_lg()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().muted)
                        .p_3()
                        .whitespace_normal()
                        .text_sm()
                        .line_height(px(22.0))
                        .child(preset.content.clone()),
                ),
        )
        .into_any_element()
}

fn title_prompt_content(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let prompt = app.settings().title_generation_system_prompt.trim();
    let customized = prompt != DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT;
    let status = div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child("Current Prompt"),
        )
        .child(status_pill(
            if customized { "Customized" } else { "Built-in" },
            customized,
            cx,
        ));

    if let Some(editor) = &app.settings_ui.title_prompt_editor {
        let mut actions = div().flex().justify_end().gap_2();
        if customized {
            actions = actions.child(
                icon_action(
                    "reset-title-generation-prompt-editor",
                    AppIcon::Regenerate,
                    IconTone::Muted,
                    "Reset to default",
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.reset_title_generation_prompt(cx))),
            );
        }
        return stretching_column()
            .w_full()
            .p_3()
            .gap_4()
            .child(status)
            .child(
                stretching_column()
                    .gap_4()
                    .child(
                        Form::vertical().child(
                            Field::new().label("Prompt").required(true).child(
                                Input::new(editor)
                                    .aria_label("Title generation prompt")
                                    .h(px(220.0)),
                            ),
                        ),
                    )
                    .child(
                        actions
                            .child(
                                icon_action(
                                    "cancel-title-generation-system-prompt",
                                    AppIcon::Close,
                                    IconTone::Muted,
                                    "Cancel",
                                    cx,
                                )
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.cancel_title_prompt_edit(cx)),
                                ),
                            )
                            .child(
                                primary_icon_action(
                                    "save-title-generation-system-prompt",
                                    AppIcon::Save,
                                    "Save title prompt",
                                    cx,
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.save_title_prompt(cx))),
                            ),
                    ),
            )
            .into_any_element();
    }

    let mut actions = div().flex_none().flex().items_center().gap_2();
    if customized {
        actions = actions.child(
            icon_action(
                "reset-title-generation-prompt",
                AppIcon::Regenerate,
                IconTone::Muted,
                "Reset to default",
                cx,
            )
            .on_click(cx.listener(|this, _, _, cx| this.reset_title_generation_prompt(cx))),
        );
    }
    actions = actions.child(
        primary_icon_action(
            "edit-title-generation-system-prompt",
            AppIcon::Pencil,
            "Edit title prompt",
            cx,
        )
        .on_click(cx.listener(|this, _, window, cx| this.begin_edit_title_prompt(window, cx))),
    );

    let preview = if prompt.is_empty() {
        "No title instructions.".to_string()
    } else {
        prompt_preview(prompt)
    };

    stretching_column()
        .w_full()
        .p_3()
        .gap_4()
        .child(status)
        .child(
            div()
                .rounded_lg()
                .bg(cx.theme().muted)
                .px_3()
                .py_3()
                .flex()
                .items_start()
                .justify_between()
                .gap_5()
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .max_h(px(54.0))
                        .overflow_hidden()
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(preview),
                )
                .child(actions),
        )
        .into_any_element()
}
