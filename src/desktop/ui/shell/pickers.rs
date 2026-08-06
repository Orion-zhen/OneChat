use super::*;

pub(super) fn render_command_palette(
    app: &OneChat,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let commands = app.filtered_commands();
    let mut rows = div()
        .id("command-palette-list")
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&app.overlays.command_scroll)
        .flex()
        .flex_col()
        .gap_1();
    if commands.is_empty() {
        rows = rows.child(
            div()
                .p_5()
                .text_sm()
                .text_color(colors.muted)
                .text_center()
                .child("No matching commands"),
        );
    } else {
        for (index, command) in commands.into_iter().enumerate() {
            let shortcut = command_shortcut(command);
            let selected = index == app.overlays.command_selection;
            rows = rows.child(
                div()
                    .id(SharedString::from(format!("command-{command:?}")))
                    .rounded_lg()
                    .bg(if selected {
                        colors.accent_soft
                    } else {
                        rgba(0x00000000)
                    })
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .hover(move |style| {
                        style.bg(if selected {
                            colors.accent_soft
                        } else {
                            colors.hover
                        })
                    })
                    .active(move |style| style.bg(colors.accent_soft))
                    .on_click(cx.listener(move |this, _, _, cx| this.execute_command(command, cx)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(
                                div()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(command.label()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(colors.muted)
                                            .child(command.detail()),
                                    ),
                            )
                            .children(shortcut.map(|shortcut| {
                                div()
                                    .flex_none()
                                    .rounded_md()
                                    .bg(colors.raised)
                                    .px_2()
                                    .py_1()
                                    .text_size(px(11.0))
                                    .text_color(colors.muted)
                                    .child(shortcut)
                            })),
                    ),
            );
        }
    }

    let panel = div()
        .w_full()
        .max_w(px(600.0))
        .max_h(px(560.0))
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .shadow_lg()
        .p_2()
        .flex()
        .flex_col()
        .gap_2()
        .child(app.overlays.command_input.clone())
        .child(rows)
        .child(
            div()
                .px_2()
                .pb_1()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(11.0))
                .text_color(colors.muted)
                .child("↑↓ Navigate   ↩ Select")
                .child("Esc Close"),
        );
    animated_overlay(
        panel,
        colors,
        "command-palette-backdrop",
        "command-palette-panel",
    )
}

fn command_shortcut(command: PaletteCommand) -> Option<String> {
    match command {
        PaletteCommand::NewConversation => Some(shortcut_label("N")),
        PaletteCommand::ChooseModel => Some(shortcut_label("L")),
        PaletteCommand::ToggleSidebar => Some(if cfg!(target_os = "macos") {
            "⇧⌘S".into()
        } else {
            "Ctrl+Shift+S".into()
        }),
        PaletteCommand::OpenSettings => Some(shortcut_label(",")),
        _ => None,
    }
}

pub(super) fn render_model_picker(
    app: &OneChat,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let current_model_id = app.selected_model().map(|model| model.id.as_str());
    let filtered_models = app.filtered_models();
    let mut models = div()
        .id("model-picker-list")
        .min_h_0()
        .flex_1()
        .overflow_y_scroll()
        .track_scroll(&app.overlays.model_scroll)
        .flex()
        .flex_col()
        .gap_1();

    if app.data.snapshot.models.is_empty() {
        models = models.child(notice_row("No models configured.", colors));
    } else if filtered_models.is_empty() {
        models = models.child(notice_row("No models match this search.", colors));
    } else {
        for (index, model) in filtered_models.into_iter().enumerate() {
            let provider = app
                .provider_for_model(model)
                .map(|provider| provider.name.as_str())
                .unwrap_or("Missing provider");
            let availability = app.model_availability(model);
            let available = availability.is_ok();
            let status = availability.map_or_else(|reason| reason, |_| "Available");
            let current = current_model_id == Some(model.id.as_str());
            let highlighted = index == app.overlays.model_selection;
            let model_id = model.id.clone();
            models = models.child(
                div()
                    .id(SharedString::from(format!("pick-model-{}", model.id)))
                    .rounded_lg()
                    .bg(if highlighted || current {
                        colors.accent_soft
                    } else {
                        rgba(0x00000000)
                    })
                    .px_3()
                    .py_3()
                    .when(available, |element| {
                        element
                            .cursor_pointer()
                            .hover(move |style| {
                                style.bg(if highlighted || current {
                                    colors.accent_soft
                                } else {
                                    colors.hover
                                })
                            })
                            .active(move |style| style.bg(colors.accent_soft))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_model(model_id.clone(), cx)
                            }))
                    })
                    .when(!available, |element| element.opacity(0.55))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .child(
                                div()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
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
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(colors.muted)
                                            .child(inspector::capability_summary(model)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_sm()
                                    .text_color(if current {
                                        colors.accent
                                    } else if available {
                                        colors.muted
                                    } else {
                                        colors.danger
                                    })
                                    .child(if current { "✓" } else { status }),
                            ),
                    ),
            );
        }
    }

    let panel = div()
        .w_full()
        .max_w(px(560.0))
        .max_h(px(640.0))
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .shadow_lg()
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .px_1()
                .pt_1()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Choose Model"),
                )
                .child(
                    icon_button("close-model-picker", "×", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.close_model_picker(cx))),
                ),
        )
        .child(app.overlays.model_search_input.clone())
        .child(models)
        .child(
            div()
                .px_1()
                .text_size(px(11.0))
                .text_color(colors.muted)
                .child("↑↓ Navigate   ↩ Select   Esc Close"),
        );
    animated_overlay(panel, colors, "model-picker-backdrop", "model-picker-panel")
}

fn notice_row(message: &str, colors: Colors) -> AnyElement {
    div()
        .rounded_lg()
        .bg(colors.raised)
        .p_4()
        .text_sm()
        .text_color(colors.muted)
        .child(message.to_string())
        .into_any_element()
}
