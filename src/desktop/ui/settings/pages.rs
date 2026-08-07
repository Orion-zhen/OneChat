use super::*;

pub(super) fn general_page(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let theme = app.settings().theme.label();
    let appearance = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(setting_row(
            "Theme",
            "Match the Mac or choose a fixed appearance.",
            button("cycle-theme", theme, colors)
                .on_click(cx.listener(|this, _, _, cx| this.cycle_theme(cx))),
            colors,
        ))
        .child(setting_row(
            "Message Width",
            "Maximum width as a share of the available chat area.",
            message_width_slider(app, colors, cx),
            colors,
        ));

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
            .child(section("Appearance", None, appearance, colors)),
    )
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
    let content = setting_row(
        "Primary Model",
        "Used when creating a new conversation.",
        primary_model_select(app, colors, scale_factor, cx),
        colors,
    );

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "Default Models",
                "Choose the models OneChat uses for new conversations.",
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

fn primary_model_select(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let selected_id = app.settings().primary_model_id.as_deref();
    let selected_model =
        selected_id.and_then(|id| app.data.snapshot.models.iter().find(|model| model.id == id));
    let label = selected_model.map_or_else(
        || "Choose a model".to_string(),
        |model| {
            if app.model_availability(model).is_ok() {
                model.display_name.clone()
            } else {
                format!("{} · Unavailable", model.display_name)
            }
        },
    );

    let mut options = div()
        .id("primary-model-options")
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
                    .id(SharedString::from(format!("primary-model-{}", model.id)))
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
                        this.select_primary_model(model_id.clone(), cx)
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
        .id("primary-model-select")
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
        .on_click(cx.listener(|this, _, _, cx| this.toggle_primary_model_menu(cx)))
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(label),
        )
        .child(svg_icon(
            if app.settings_ui.default_model_menu_open {
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
            app.settings_ui
                .default_model_menu_open
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
    let edit_actions = div()
        .flex()
        .justify_end()
        .gap_2()
        .child(
            large_svg_icon_button(
                "cancel-default-system-prompt",
                UiIcon::Close,
                IconTone::Muted,
                colors,
                scale_factor,
            )
            .on_click(cx.listener(|this, _, _, cx| this.cancel_default_system_prompt_edit(cx))),
        )
        .child(
            primary_svg_icon_button(
                "save-default-system-prompt",
                UiIcon::Save,
                colors,
                scale_factor,
            )
            .on_click(cx.listener(|this, _, _, cx| this.save_default_system_prompt(cx))),
        );
    let content = if let Some(editor) = &app.settings_ui.default_system_prompt_editor {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(editor.clone())
            .child(edit_actions)
            .into_any_element()
    } else {
        let prompt = app.data.snapshot.settings.default_system_prompt.trim();
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
                            .child("Default Prompt"),
                    )
                    .child(
                        div()
                            .pt_1()
                            .text_size(px(12.0))
                            .line_height(px(18.0))
                            .text_color(colors.muted)
                            .child(if prompt.is_empty() {
                                "New conversations start without a System Prompt.".into()
                            } else {
                                prompt_preview(prompt)
                            }),
                    ),
            )
            .child(
                button("edit-default-system-prompt", "Edit", colors).on_click(
                    cx.listener(|this, _, _, cx| this.begin_edit_default_system_prompt(cx)),
                ),
            )
            .into_any_element()
    };

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "System Prompts",
                "Set the instructions copied into every new conversation.",
                colors,
            ))
            .child(section(
                "Default",
                Some("Existing conversations keep their own prompt."),
                content,
                colors,
            )),
    )
}
