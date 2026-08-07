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
        ("theme-system", Theme::System, Icon::Monitor),
        ("theme-light", Theme::Light, Icon::Sun),
        ("theme-dark", Theme::Dark, Icon::Moon),
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
                .child(render_icon(
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
                .children(selected.then(|| {
                    render_icon(Icon::Check, IconTone::Accent, colors, scale_factor, 14.0)
                })),
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
                    .children(selected.then(|| {
                        render_icon(Icon::Check, IconTone::Accent, colors, scale_factor, 14.0)
                    })),
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
        .child(render_icon(
            if app.settings_ui.default_model_menu == Some(role) {
                Icon::ChevronUp
            } else {
                Icon::ChevronDown
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
    let preset_count = app.data.snapshot.prompt_presets.len();
    let preset_count_label = format!(
        "{preset_count} {}",
        if preset_count == 1 {
            "preset"
        } else {
            "presets"
        }
    );

    let conversation_default = div()
        .p_5()
        .flex()
        .items_center()
        .justify_between()
        .gap_6()
        .child(
            div()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Conversation default"),
                        )
                        .child(prompt_badge("New chats", false, colors)),
                )
                .child(
                    div()
                        .pt_1()
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .text_color(colors.muted)
                        .child("Choose the preset copied into each new conversation."),
                ),
        )
        .child(default_prompt_select(app, colors, scale_factor, cx));

    let conversation_prompt = stretching_column()
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .child(conversation_default);

    let title_prompt = stretching_column()
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .p_5()
        .child(title_prompt_content(app, colors, scale_factor, cx));

    let prompt_library_header = div()
        .pb_4()
        .flex()
        .items_start()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Prompt library"),
                        )
                        .child(prompt_badge(preset_count_label, false, colors)),
                )
                .child(
                    div()
                        .pt_1()
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .text_color(colors.muted)
                        .child("Reusable Markdown prompts for conversations."),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    large_icon_button(
                        "reload-prompt-presets",
                        Icon::Regenerate,
                        IconTone::Muted,
                        colors,
                        scale_factor,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.reload_prompt_presets(cx))),
                )
                .child(
                    primary_icon_button("add-prompt-preset", Icon::Plus, colors, scale_factor)
                        .on_click(cx.listener(|this, _, _, cx| this.begin_add_prompt_preset(cx))),
                ),
        );

    let prompt_library = stretching_column()
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .p_5()
        .child(prompt_library_header)
        .child(prompt_presets_content(app, colors, scale_factor, cx));

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "System Prompts",
                "Set the instructions used in conversations and automatic titles.",
                colors,
            ))
            .child(conversation_prompt)
            .child(title_prompt)
            .child(prompt_library),
    )
}

fn prompt_badge(label: impl Into<SharedString>, accent: bool, colors: Colors) -> AnyElement {
    div()
        .flex_none()
        .rounded_full()
        .bg(if accent {
            colors.accent_soft
        } else {
            colors.raised
        })
        .px_2()
        .py_1()
        .text_size(px(10.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if accent { colors.accent } else { colors.muted })
        .child(label.into())
        .into_any_element()
}

fn stretching_column() -> Div {
    let mut column = div().flex().flex_col();
    column.style().align_items = Some(AlignItems::Stretch);
    column
}

fn default_prompt_select(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let selected_name = app.settings().default_system_prompt_preset.as_deref();
    let label = selected_name.map_or_else(
        || "No System Prompt".to_string(),
        |name| {
            if app.prompt_preset(name).is_some() {
                name.to_string()
            } else {
                format!("Missing · {name}")
            }
        },
    );
    let none_selected = selected_name.is_none();
    let mut options = div()
        .id("default-prompt-options")
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
        .shadow_lg()
        .child(
            div()
                .id("default-prompt-none")
                .w_full()
                .px_3()
                .py_2()
                .rounded_md()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .bg(if none_selected {
                    colors.accent_soft
                } else {
                    colors.panel
                })
                .cursor_pointer()
                .hover(move |style| style.bg(colors.hover))
                .on_click(cx.listener(|this, _, _, cx| this.select_default_prompt(None, cx)))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("No System Prompt"),
                )
                .children(none_selected.then(|| {
                    render_icon(Icon::Check, IconTone::Accent, colors, scale_factor, 14.0)
                })),
        );

    for preset in &app.data.snapshot.prompt_presets {
        let name = preset.name.clone();
        let selected = selected_name == Some(preset.name.as_str());
        options = options.child(
            div()
                .id(SharedString::from(format!(
                    "default-prompt-{}",
                    preset.name
                )))
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
                    this.select_default_prompt(Some(name.clone()), cx)
                }))
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
                .children(selected.then(|| {
                    render_icon(Icon::Check, IconTone::Accent, colors, scale_factor, 14.0)
                })),
        );
    }

    let select = div()
        .id("default-prompt-select")
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
        .on_click(cx.listener(|this, _, _, cx| this.toggle_default_prompt_menu(cx)))
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(label),
        )
        .child(render_icon(
            if app.settings_ui.default_prompt_menu_open {
                Icon::ChevronUp
            } else {
                Icon::ChevronDown
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
            app.settings_ui
                .default_prompt_menu_open
                .then(|| deferred(options).priority(1)),
        )
        .into_any_element()
}

fn prompt_presets_content(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    if app.data.snapshot.prompt_presets.is_empty() {
        return div()
            .rounded_lg()
            .bg(colors.raised)
            .px_4()
            .py_5()
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
                    .text_color(colors.muted)
                    .child("Create a reusable prompt to get started."),
            )
            .into_any_element();
    }

    let mut cards = div().flex().flex_wrap().gap_3();
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
                .w(px(350.0))
                .max_w_full()
                .min_h(px(132.0))
                .rounded_lg()
                .border_1()
                .border_color(colors.border)
                .bg(colors.raised)
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
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
                                .children(default.then(|| prompt_badge("Default", true, colors))),
                        )
                        .child(
                            div()
                                .flex_none()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    icon_button(
                                        SharedString::from(format!("view-prompt-{}", preset.name)),
                                        Icon::Eye,
                                        IconTone::Muted,
                                        colors,
                                        scale_factor,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.view_prompt_preset(view_name.clone(), cx)
                                        },
                                    )),
                                )
                                .child(
                                    icon_button(
                                        SharedString::from(format!("edit-prompt-{}", preset.name)),
                                        Icon::Pencil,
                                        IconTone::Muted,
                                        colors,
                                        scale_factor,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.begin_edit_prompt_preset(edit_name.clone(), cx)
                                        },
                                    )),
                                )
                                .child(
                                    icon_button(
                                        SharedString::from(format!(
                                            "delete-prompt-{}",
                                            preset.name
                                        )),
                                        Icon::Trash,
                                        IconTone::Danger,
                                        colors,
                                        scale_factor,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.request_delete_prompt_preset(
                                                delete_name.clone(),
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
                        .max_h(px(54.0))
                        .overflow_hidden()
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .text_color(colors.muted)
                        .child(prompt_preview(&preset.content)),
                ),
        );
    }
    cards.into_any_element()
}

pub(crate) fn prompt_preset_panel(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> Div {
    if let Some(editor) = &app.settings_ui.prompt_preset_editor {
        let title = if editor.original_name().is_some() {
            "Edit prompt preset"
        } else {
            "New prompt preset"
        };
        let actions = div()
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .child(
                large_icon_button(
                    "cancel-prompt-preset",
                    Icon::Close,
                    IconTone::Muted,
                    colors,
                    scale_factor,
                )
                .on_click(cx.listener(|this, _, _, cx| this.cancel_prompt_preset_edit(cx))),
            )
            .child(
                primary_icon_button("save-prompt-preset", Icon::Save, colors, scale_factor)
                    .on_click(cx.listener(|this, _, _, cx| this.save_prompt_preset(cx))),
            );
        let body = stretching_column()
            .gap_4()
            .child(field("Name", editor.name.clone(), colors))
            .child(field("Prompt", editor.content.clone(), colors))
            .children(
                app.settings_ui
                    .form_error
                    .as_deref()
                    .map(|error| error_banner(error, colors)),
            );
        return prompt_preset_modal(title, actions, body, colors);
    }

    let preset = app
        .settings_ui
        .viewed_prompt_preset
        .as_deref()
        .and_then(|name| app.prompt_preset(name))
        .expect("prompt preset panel requires a viewed preset or editor");
    let actions = div().flex_none().child(
        large_icon_button(
            "close-prompt-preset-view",
            Icon::Close,
            IconTone::Muted,
            colors,
            scale_factor,
        )
        .on_click(cx.listener(|this, _, _, cx| this.close_prompt_preset_view(cx))),
    );
    let body = stretching_column()
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
                        .text_color(colors.muted)
                        .child("Name"),
                )
                .child(
                    div()
                        .rounded_lg()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.raised)
                        .p_3()
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
                        .text_color(colors.muted)
                        .child("Prompt"),
                )
                .child(
                    div()
                        .id("viewed-prompt-content")
                        .min_h(px(240.0))
                        .max_h(px(420.0))
                        .overflow_y_scroll()
                        .rounded_lg()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.raised)
                        .p_3()
                        .whitespace_normal()
                        .text_sm()
                        .line_height(px(22.0))
                        .child(preset.content.clone()),
                ),
        );
    prompt_preset_modal("View prompt preset", actions, body, colors)
}

fn prompt_preset_modal(title: &str, actions: Div, body: Div, colors: Colors) -> Div {
    stretching_column()
        .w_full()
        .max_w(px(680.0))
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .shadow_lg()
        .p_5()
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.to_string()),
                )
                .child(actions),
        )
        .child(body)
}

fn title_prompt_content(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let prompt = app.settings().title_generation_system_prompt.trim();
    let customized = prompt != DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT;
    let heading = div()
        .flex()
        .items_start()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Title generation"),
                )
                .child(
                    div()
                        .pt_1()
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .text_color(colors.muted)
                        .child("Instructions used after the first completed response."),
                ),
        )
        .child(prompt_badge(
            if customized { "Customized" } else { "Built-in" },
            customized,
            colors,
        ));

    if let Some(editor) = &app.settings_ui.title_prompt_editor {
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
        return stretching_column()
            .gap_4()
            .child(heading)
            .child(
                stretching_column()
                    .rounded_lg()
                    .bg(colors.raised)
                    .p_4()
                    .gap_4()
                    .child(editor.clone())
                    .child(
                        actions
                            .child(
                                large_icon_button(
                                    "cancel-title-generation-system-prompt",
                                    Icon::Close,
                                    IconTone::Muted,
                                    colors,
                                    scale_factor,
                                )
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.cancel_title_prompt_edit(cx)),
                                ),
                            )
                            .child(
                                primary_icon_button(
                                    "save-title-generation-system-prompt",
                                    Icon::Save,
                                    colors,
                                    scale_factor,
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
            button("reset-title-generation-prompt", "Reset to Default", colors)
                .on_click(cx.listener(|this, _, _, cx| this.reset_title_generation_prompt(cx))),
        );
    }
    actions = actions.child(
        icon_button(
            "edit-title-generation-system-prompt",
            Icon::Pencil,
            IconTone::Muted,
            colors,
            scale_factor,
        )
        .on_click(cx.listener(|this, _, _, cx| this.begin_edit_title_prompt(cx))),
    );

    let preview = if prompt.is_empty() {
        "No title instructions.".to_string()
    } else {
        prompt_preview(prompt)
    };

    stretching_column()
        .gap_4()
        .child(heading)
        .child(
            div()
                .rounded_lg()
                .bg(colors.raised)
                .p_4()
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
                        .text_color(colors.muted)
                        .child(preview),
                )
                .child(actions),
        )
        .into_any_element()
}
