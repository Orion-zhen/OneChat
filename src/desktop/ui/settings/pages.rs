use super::*;

pub(super) fn general_page(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let appearance = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(setting_row(
            "Theme",
            "Match the Mac or choose a fixed appearance.",
            theme_selector(app, colors, scale_factor, cx),
            colors,
        ))
        .child(setting_row(
            "Message Width",
            "Maximum width as a share of the available chat area.",
            message_width_slider(app, colors, cx),
            colors,
        ));
    let automation = setting_row(
        "Automatic Titles",
        "Generate a title after the first completed response.",
        auto_title_toggle(app, colors, cx),
        colors,
    );

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "General",
                "Choose how OneChat looks and responds.",
                colors,
            ))
            .child(section("Appearance", None, appearance, colors))
            .child(section("Automation", None, automation, colors)),
    )
}

fn auto_title_toggle(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let enabled = app.settings().auto_title_enabled;
    div()
        .id("automatic-titles-toggle")
        .w(px(32.0))
        .h(px(18.0))
        .p(px(2.0))
        .flex_none()
        .rounded_full()
        .border_1()
        .border_color(if enabled {
            colors.accent
        } else {
            colors.border
        })
        .bg(if enabled {
            colors.accent
        } else {
            colors.raised
        })
        .flex()
        .items_center()
        .when(enabled, |element| element.justify_end())
        .cursor_pointer()
        .hover(|style| style.opacity(0.8))
        .on_click(cx.listener(|this, _, _, cx| this.toggle_auto_title_enabled(cx)))
        .child(div().size(px(12.0)).rounded_full().bg(if enabled {
            colors.on_accent
        } else {
            colors.muted
        }))
        .into_any_element()
}

fn theme_selector(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let mut segments = div()
        .flex_none()
        .flex()
        .gap_1()
        .rounded_lg()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .p(px(2.0));

    for (id, theme, icon) in [
        ("theme-system", Theme::System, UiIcon::Monitor),
        ("theme-light", Theme::Light, UiIcon::Sun),
        ("theme-dark", Theme::Dark, UiIcon::Moon),
    ] {
        let selected = app.settings().theme == theme;
        segments = segments.child(
            div()
                .id(id)
                .w(px(40.0))
                .h(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .bg(if selected {
                    colors.accent_soft
                } else {
                    rgba(0x00000000)
                })
                .cursor_pointer()
                .hover(move |style| {
                    style.bg(if selected {
                        colors.accent_soft
                    } else {
                        colors.hover
                    })
                })
                .active(move |style| style.bg(colors.accent_soft))
                .child(svg_icon(
                    icon,
                    if selected {
                        IconTone::Accent
                    } else {
                        IconTone::Muted
                    },
                    colors,
                    scale_factor,
                    16.0,
                ))
                .on_click(cx.listener(move |this, _, _, cx| this.set_theme(theme, cx))),
        );
    }

    segments.into_any_element()
}

const MESSAGE_WIDTH_SLIDER_WIDTH: f32 = 180.0;
const MESSAGE_WIDTH_THUMB_SIZE: f32 = 16.0;

fn message_width_slider(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let ratio = app.settings().message_width_ratio();
    let progress =
        (ratio - MIN_MESSAGE_WIDTH_RATIO) / (MAX_MESSAGE_WIDTH_RATIO - MIN_MESSAGE_WIDTH_RATIO);
    let track_width = MESSAGE_WIDTH_SLIDER_WIDTH - MESSAGE_WIDTH_THUMB_SIZE;
    let entity = cx.entity();

    let input = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            window.on_mouse_event({
                let entity = entity.clone();
                move |event: &MouseDownEvent, _, _, cx| {
                    if event.button != MouseButton::Left || !bounds.contains(&event.position) {
                        return;
                    }
                    let ratio = message_width_ratio_at(event.position.x, bounds);
                    entity.update(cx, |this, cx| this.begin_message_width_drag(ratio, cx));
                }
            });
            window.on_mouse_event({
                let entity = entity.clone();
                move |event: &MouseMoveEvent, _, _, cx| {
                    if !event.dragging() || !entity.read(cx).settings_ui.message_width_dragging {
                        return;
                    }
                    let ratio = message_width_ratio_at(event.position.x, bounds);
                    entity.update(cx, |this, cx| this.update_message_width_ratio(ratio, cx));
                }
            });
            window.on_mouse_event(move |event: &MouseUpEvent, _, _, cx| {
                if event.button == MouseButton::Left {
                    entity.update(cx, |this, cx| this.finish_message_width_drag(cx));
                }
            });
        },
    )
    .absolute()
    .top_0()
    .right_0()
    .bottom_0()
    .left_0();

    div()
        .flex_none()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .relative()
                .w(px(MESSAGE_WIDTH_SLIDER_WIDTH))
                .h(px(28.0))
                .cursor_pointer()
                .child(
                    div()
                        .absolute()
                        .left(px(MESSAGE_WIDTH_THUMB_SIZE / 2.0))
                        .right(px(MESSAGE_WIDTH_THUMB_SIZE / 2.0))
                        .top(px(12.0))
                        .h(px(4.0))
                        .rounded_full()
                        .bg(colors.border),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(MESSAGE_WIDTH_THUMB_SIZE / 2.0))
                        .top(px(12.0))
                        .w(px(track_width * progress))
                        .h(px(4.0))
                        .rounded_full()
                        .bg(colors.accent),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(track_width * progress))
                        .top(px(6.0))
                        .size(px(MESSAGE_WIDTH_THUMB_SIZE))
                        .rounded_full()
                        .border_1()
                        .border_color(colors.accent)
                        .bg(colors.panel)
                        .shadow_sm(),
                )
                .child(input),
        )
        .child(
            div()
                .w(px(42.0))
                .flex_none()
                .text_right()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.accent)
                .child(format!("{:.0}%", ratio * 100.0)),
        )
        .into_any_element()
}

fn message_width_ratio_at(x: Pixels, bounds: Bounds<Pixels>) -> f32 {
    let inset = px(MESSAGE_WIDTH_THUMB_SIZE / 2.0);
    let progress =
        ((x - bounds.left() - inset) / (bounds.size.width - inset * 2.0)).clamp(0.0, 1.0);
    let ratio =
        MIN_MESSAGE_WIDTH_RATIO + progress * (MAX_MESSAGE_WIDTH_RATIO - MIN_MESSAGE_WIDTH_RATIO);
    (ratio * 100.0).round() / 100.0
}

pub(super) fn default_models_page(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let content = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(setting_row(
            "Primary Model",
            "Used when creating a new conversation.",
            default_model_select(app, DefaultModelRole::Primary, colors, scale_factor, cx),
            colors,
        ))
        .child(setting_row(
            "Title Generation Model",
            "Used to generate an automatic title after the first response.",
            default_model_select(
                app,
                DefaultModelRole::TitleGeneration,
                colors,
                scale_factor,
                cx,
            ),
            colors,
        ));

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "Default Models",
                "Choose the models OneChat uses by default.",
                colors,
            ))
            .child(section(
                "Models",
                Some("Only available models can be selected."),
                content,
                colors,
            )),
    )
}

fn default_model_select(
    app: &OneChat,
    role: DefaultModelRole,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let selected_id = match role {
        DefaultModelRole::Primary => app.settings().primary_model_id.as_deref(),
        DefaultModelRole::TitleGeneration => app.settings().title_generation_model_id.as_deref(),
    };
    let selected_model =
        selected_id.and_then(|id| app.data.snapshot.models.iter().find(|model| model.id == id));
    let label = selected_model.map_or_else(
        || match role {
            DefaultModelRole::Primary => "Choose a model".to_string(),
            DefaultModelRole::TitleGeneration => "Use Primary Model".to_string(),
        },
        |model| {
            if app.model_availability(model).is_ok() {
                model.display_name.clone()
            } else {
                format!("{} · Unavailable", model.display_name)
            }
        },
    );

    let key = match role {
        DefaultModelRole::Primary => "primary",
        DefaultModelRole::TitleGeneration => "title-generation",
    };
    let mut options = div()
        .id(SharedString::from(format!("{key}-model-options")))
        .occlude()
        .absolute()
        .top(px(40.0))
        .left_0()
        .right_0()
        .max_h(px(320.0))
        .overflow_y_scroll()
        .rounded_lg()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .p_1()
        .flex()
        .flex_col()
        .shadow_lg();
    if role == DefaultModelRole::TitleGeneration {
        let selected = selected_id.is_none();
        options = options.child(
            div()
                .id("title-generation-model-primary")
                .w_full()
                .px_3()
                .py_2()
                .rounded_md()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .bg(if selected {
                    colors.accent_soft
                } else {
                    colors.panel
                })
                .cursor_pointer()
                .hover(move |style| style.bg(colors.hover))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_default_model(DefaultModelRole::TitleGeneration, None, cx)
                }))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Use Primary Model"),
                )
                .children(selected.then(|| div().flex_none().text_color(colors.accent).child("✓"))),
        );
    }
    let available_models = app
        .data
        .snapshot
        .models
        .iter()
        .filter(|model| app.model_availability(model).is_ok())
        .collect::<Vec<_>>();
    if available_models.is_empty() {
        options = options.child(
            div()
                .px_3()
                .py_2()
                .text_sm()
                .text_color(colors.muted)
                .child("No available models configured."),
        );
    } else {
        for model in available_models {
            let model_id = model.id.clone();
            let selected = selected_id == Some(model.id.as_str());
            let provider = app
                .provider_for_model(model)
                .map(|provider| provider.name.as_str())
                .unwrap_or("Missing provider");
            options = options.child(
                div()
                    .id(SharedString::from(format!("{key}-model-{}", model.id)))
                    .w_full()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .bg(if selected {
                        colors.accent_soft
                    } else {
                        colors.panel
                    })
                    .cursor_pointer()
                    .hover(move |style| style.bg(colors.hover))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_default_model(role, Some(model_id.clone()), cx)
                    }))
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                div()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(model.display_name.clone()),
                            )
                            .child(
                                div()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_size(px(11.0))
                                    .text_color(colors.muted)
                                    .child(format!("{} · {provider}", model.remote_id)),
                            ),
                    )
                    .children(
                        selected.then(|| div().flex_none().text_color(colors.accent).child("✓")),
                    ),
            );
        }
    }

    let select = div()
        .id(SharedString::from(format!("{key}-model-select")))
        .w_full()
        .h(px(36.0))
        .px_3()
        .rounded_lg()
        .border_1()
        .border_color(colors.border)
        .bg(colors.raised)
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .text_sm()
        .cursor_pointer()
        .hover(move |style| style.bg(colors.hover))
        .on_click(cx.listener(move |this, _, _, cx| this.toggle_default_model_menu(role, cx)))
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(label),
        )
        .child(svg_icon(
            if app.settings_ui.default_model_menu == Some(role) {
                UiIcon::ChevronUp
            } else {
                UiIcon::ChevronDown
            },
            IconTone::Muted,
            colors,
            scale_factor,
            14.0,
        ));

    div()
        .relative()
        .w(px(300.0))
        .flex_none()
        .child(select)
        .children(
            (app.settings_ui.default_model_menu == Some(role))
                .then(|| deferred(options).priority(1)),
        )
        .into_any_element()
}

pub(super) fn system_prompts_page(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let conversation_prompt = settings_prompt_content(
        app,
        SettingsPromptKind::ConversationDefault,
        "Default Prompt",
        "New conversations start without a System Prompt.",
        colors,
        scale_factor,
        cx,
    );
    let title_prompt = settings_prompt_content(
        app,
        SettingsPromptKind::TitleGeneration,
        "Title Prompt",
        "",
        colors,
        scale_factor,
        cx,
    );

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "System Prompts",
                "Configure instructions for conversations and automatic titles.",
                colors,
            ))
            .child(section(
                "Conversation Default",
                Some(
                    "Copied into new conversations; existing conversations keep their own prompt.",
                ),
                conversation_prompt,
                colors,
            ))
            .child(section(
                "Title Generation",
                Some("Used after the first completed response; changes apply to future titles."),
                title_prompt,
                colors,
            )),
    )
}

fn settings_prompt_content(
    app: &OneChat,
    kind: SettingsPromptKind,
    title: &'static str,
    empty_message: &'static str,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let key = match kind {
        SettingsPromptKind::ConversationDefault => "default",
        SettingsPromptKind::TitleGeneration => "title-generation",
    };
    let prompt = match kind {
        SettingsPromptKind::ConversationDefault => {
            app.data.snapshot.settings.default_system_prompt.trim()
        }
        SettingsPromptKind::TitleGeneration => app
            .data
            .snapshot
            .settings
            .title_generation_system_prompt
            .trim(),
    };
    let customized = kind == SettingsPromptKind::TitleGeneration
        && prompt != DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT;

    if let Some(editor) = app
        .settings_ui
        .prompt_editor
        .as_ref()
        .filter(|editor| editor.kind == kind)
    {
        let mut actions = div().flex().justify_end().gap_2();
        if customized {
            actions = actions.child(
                button(
                    "reset-title-generation-prompt-editor",
                    "Reset to Default",
                    colors,
                )
                .on_click(cx.listener(|this, _, _, cx| this.reset_title_generation_prompt(cx))),
            );
        }
        actions = actions
            .child(
                large_svg_icon_button(
                    SharedString::from(format!("cancel-{key}-system-prompt")),
                    UiIcon::Close,
                    IconTone::Muted,
                    colors,
                    scale_factor,
                )
                .on_click(cx.listener(|this, _, _, cx| this.cancel_settings_prompt_edit(cx))),
            )
            .child(
                primary_svg_icon_button(
                    SharedString::from(format!("save-{key}-system-prompt")),
                    UiIcon::Save,
                    colors,
                    scale_factor,
                )
                .on_click(cx.listener(|this, _, _, cx| this.save_settings_prompt(cx))),
            );
        return div()
            .flex()
            .flex_col()
            .gap_4()
            .child(editor.input.clone())
            .child(actions)
            .into_any_element();
    }

    let kind_for_edit = kind;
    let mut actions = div().flex().items_center().gap_2();
    if customized {
        actions = actions.child(
            button("reset-title-generation-prompt", "Reset to Default", colors)
                .on_click(cx.listener(|this, _, _, cx| this.reset_title_generation_prompt(cx))),
        );
    }
    actions = actions.child(
        button(
            SharedString::from(format!("edit-{key}-system-prompt")),
            "Edit",
            colors,
        )
        .on_click(
            cx.listener(move |this, _, _, cx| this.begin_edit_settings_prompt(kind_for_edit, cx)),
        ),
    );

    div()
        .flex()
        .items_start()
        .justify_between()
        .gap_5()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .pt_1()
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .text_color(colors.muted)
                        .child(if prompt.is_empty() {
                            empty_message.to_string()
                        } else {
                            prompt_preview(prompt)
                        }),
                ),
        )
        .child(actions)
        .into_any_element()
}
