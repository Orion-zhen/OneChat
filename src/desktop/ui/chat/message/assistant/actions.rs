use super::*;

pub(super) fn render_message_actions(
    app: &OneChat,
    turn: &Turn,
    message: &AssistantResponse,
    latest: bool,
    generating: bool,
    action_group: SharedString,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let editing = app.assistant_message_editor(message).is_some();
    let editing_output = app.assistant_output_editing(message);
    let editing_any = app.active_message_editor().is_some();
    let has_info = app.request_for_response(message).is_some();
    let has_content = !message.content.is_empty();
    let can_copy = has_content;
    let can_edit = has_content && !generating && (!editing_any || editing_output);
    let can_regenerate = latest
        && !generating
        && !editing
        && !matches!(
            message.status,
            MessageStatus::Failed | MessageStatus::Interrupted
        );
    let can_continue = latest
        && turn.continuation_response_id.as_deref() == Some(message.id.as_str())
        && has_content
        && !generating
        && !editing_any
        && !matches!(
            message.status,
            MessageStatus::Pending | MessageStatus::Streaming
        );
    let usable_as_context = message.is_usable_as_context();
    let can_use_context = !generating
        && usable_as_context
        && turn.continuation_response_id.as_deref() != Some(&message.id);
    let can_fork = !editing_any && usable_as_context;
    let can_export = !editing_any && has_content && !(latest && generating);

    let content_actions = if can_copy || can_edit {
        let mut group = div().flex().items_center().gap_1();
        if can_copy {
            group = group.child(CopyButton::new(
                SharedString::from(format!("copy-message-{}", message.id)),
                message.content.clone(),
            ));
        }
        if can_edit {
            let edit_id = message.id.clone();
            group = group.child(
                icon_button(
                    SharedString::from(format!("edit-message-{}", message.id)),
                    AppIcon::Pencil,
                    if editing_output {
                        IconTone::Accent
                    } else {
                        IconTone::Muted
                    },
                    cx,
                )
                .selected(editing_output)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.begin_edit_assistant_output(edit_id.clone(), window, cx)
                })),
            );
        }
        Some(group)
    } else {
        None
    };

    let response_actions = if can_regenerate || can_continue || can_use_context {
        let mut group = div().flex().items_center().gap_1();
        if can_regenerate {
            let regenerate_id = message.id.clone();
            group = group.child(
                icon_button(
                    SharedString::from(format!("regenerate-message-{}", message.id)),
                    AppIcon::Regenerate,
                    IconTone::Muted,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.regenerate_assistant(regenerate_id.clone(), cx)
                })),
            );
        }
        if can_continue {
            let continue_id = message.id.clone();
            group = group.child(
                icon_button(
                    SharedString::from(format!("continue-message-{}", message.id)),
                    AppIcon::Continue,
                    IconTone::Muted,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.continue_assistant(continue_id.clone(), cx)
                })),
            );
        }
        if can_use_context {
            let context_turn_id = turn.id.clone();
            let context_response_id = message.id.clone();
            group = group.child(
                icon_button(
                    SharedString::from(format!("use-response-context-{}", message.id)),
                    AppIcon::ContextSelect,
                    IconTone::Muted,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.use_response_for_context(
                        context_turn_id.clone(),
                        context_response_id.clone(),
                        cx,
                    )
                })),
            );
        }
        Some(group)
    } else {
        None
    };

    let conversation_actions = if can_fork || can_export {
        let mut group = div().flex().items_center().gap_1();
        if can_fork {
            let fork_id = message.id.clone();
            group = group.child(
                icon_button(
                    SharedString::from(format!("fork-message-{}", message.id)),
                    AppIcon::Fork,
                    IconTone::Muted,
                    cx,
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.fork_from_response(fork_id.clone(), cx)),
                ),
            );
        }
        if can_export {
            group = group.child(export_popover(message, latest, cx));
        }
        Some(group)
    } else {
        None
    };

    let info_actions = if has_info {
        let info_id = message.id.clone();
        Some(
            div().flex().items_center().gap_1().child(
                icon_button(
                    SharedString::from(format!("info-message-{}", message.id)),
                    AppIcon::Info,
                    IconTone::Muted,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.inspect_message_request(info_id.clone(), cx)
                })),
            ),
        )
    } else {
        None
    };

    div()
        .invisible()
        .group_hover(action_group.clone(), |actions| actions.visible())
        .flex()
        .items_center()
        .gap_2()
        .children(content_actions)
        .children(response_actions)
        .children(conversation_actions)
        .children(info_actions)
        .into_any_element()
}

fn export_popover(
    message: &AssistantResponse,
    latest: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let response_id = message.id.clone();
    let app = cx.entity();
    let trigger = icon_button(
        SharedString::from(format!("export-conversation-{}", message.id)),
        AppIcon::Export,
        IconTone::Muted,
        cx,
    );

    Popover::new(SharedString::from(format!(
        "export-conversation-popover-{}",
        message.id
    )))
    .anchor(Anchor::TopRight)
    .appearance(false)
    .trigger(trigger)
    .content(move |_, _, cx| {
        let popover = cx.entity();

        let copy_markdown = {
            let popover = popover.clone();
            let app = app.clone();
            let response_id = response_id.clone();
            export_menu_button(
                "copy-conversation-markdown",
                AppIcon::Copy,
                "Copy as Markdown",
                cx,
            )
            .on_click(move |_, window, cx| {
                popover.update(cx, |popover, cx| popover.dismiss(window, cx));
                app.update(cx, |app, cx| {
                    app.copy_conversation_markdown(&response_id, window, cx)
                });
            })
        };

        let copy_png = {
            let popover = popover.clone();
            let app = app.clone();
            let response_id = response_id.clone();
            export_menu_button("copy-conversation-png", AppIcon::Image, "Copy as PNG", cx).on_click(
                move |_, window, cx| {
                    popover.update(cx, |popover, cx| popover.dismiss(window, cx));
                    app.update(cx, |app, cx| {
                        app.copy_conversation_png(&response_id, window, cx)
                    });
                },
            )
        };

        let export_markdown = {
            let popover = popover.clone();
            let app = app.clone();
            let response_id = response_id.clone();
            export_menu_button(
                "export-conversation-markdown",
                AppIcon::FileText,
                "Export Markdown",
                cx,
            )
            .on_click(move |_, window, cx| {
                popover.update(cx, |popover, cx| popover.dismiss(window, cx));
                app.update(cx, |app, cx| {
                    app.export_conversation_markdown(&response_id, window, cx)
                });
            })
        };

        let export_html = {
            let popover = popover.clone();
            let app = app.clone();
            let response_id = response_id.clone();
            export_menu_button(
                "export-conversation-html",
                AppIcon::Braces,
                "Export HTML",
                cx,
            )
            .on_click(move |_, window, cx| {
                popover.update(cx, |popover, cx| popover.dismiss(window, cx));
                app.update(cx, |app, cx| {
                    app.export_conversation_html(&response_id, window, cx)
                });
            })
        };

        let export_png = {
            let popover = popover.clone();
            let app = app.clone();
            let response_id = response_id.clone();
            export_menu_button("export-conversation-png", AppIcon::Image, "Export PNG", cx)
                .on_click(move |_, window, cx| {
                    popover.update(cx, |popover, cx| popover.dismiss(window, cx));
                    app.update(cx, |app, cx| {
                        app.export_conversation_png(&response_id, window, cx)
                    });
                })
        };

        let export_archive = latest.then(|| {
            let popover = popover.clone();
            let app = app.clone();
            let response_id = response_id.clone();
            export_menu_button(
                "export-conversation-archive",
                AppIcon::Archive,
                "Export Full Archive",
                cx,
            )
            .on_click(move |_, window, cx| {
                popover.update(cx, |popover, cx| popover.dismiss(window, cx));
                app.update(cx, |app, cx| {
                    app.export_conversation_archive(&response_id, window, cx)
                });
            })
        });

        let palette = *crate::desktop::ui::theme::palette(cx);
        let divider = || {
            div()
                .mx_3()
                .my(px(6.0))
                .h(px(1.0))
                .bg(palette.floating_border)
        };
        let panel = div()
            .w(px(296.0))
            .p(px(8.0))
            .rounded(px(14.0))
            .border_1()
            .border_color(palette.floating_border)
            .bg(palette.floating_glass)
            .shadow(vec![BoxShadow {
                color: palette.floating_shadow,
                offset: point(px(0.0), px(9.0)),
                blur_radius: px(28.0),
                spread_radius: px(-8.0),
                inset: false,
            }])
            .child(copy_markdown)
            .child(copy_png)
            .child(divider())
            .child(export_markdown)
            .child(export_html)
            .child(export_png)
            .children(export_archive);

        if cx.reduce_motion() {
            panel.into_any_element()
        } else {
            div()
                .relative()
                .child(panel)
                .with_animation(
                    SharedString::from(format!("export-popover-enter-{response_id}")),
                    Animation::new(Duration::from_millis(140)).with_easing(ease_out_quint()),
                    |panel, delta| {
                        panel
                            .opacity(0.84 + delta * 0.16)
                            .top(px(3.0 * (1.0 - delta)))
                    },
                )
                .into_any_element()
        }
    })
    .into_any_element()
}

fn export_menu_button(
    id: impl Into<ElementId>,
    icon: AppIcon,
    label: &'static str,
    cx: &App,
) -> Button {
    Button::new(id)
        .ghost()
        .w_full()
        .h(px(40.0))
        .px_3()
        .rounded(px(9.0))
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_3()
                .child(render_icon(icon, IconTone::Muted, 17.0, cx))
                .child(
                    div()
                        .whitespace_nowrap()
                        .text_size(px(14.0))
                        .line_height(px(19.0))
                        .font_weight(FontWeight::NORMAL)
                        .child(label),
                ),
        )
}
