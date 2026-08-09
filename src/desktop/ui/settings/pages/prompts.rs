use super::super::*;

pub(in crate::desktop::ui::settings) fn system_prompts_page(
    app: &OneChat,
    cx: &mut Context<OneChat>,
) -> AnyElement {
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

pub(in crate::desktop::ui::settings) fn prompt_preset_dialog_body(
    app: &OneChat,
    cx: &App,
) -> AnyElement {
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
