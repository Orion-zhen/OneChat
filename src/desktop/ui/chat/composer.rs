use super::*;

pub(super) fn render_composer(
    app: &OneChat,
    message_max_width: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let generating = app.is_current_generating();
    let multiline = composer_is_multiline(app, message_max_width, cx);
    let expanded = multiline && app.chat.composer_expanded.get();
    let invalid_for_model = !app.attachment_context_supported();
    let can_send = (!app.chat.composer.read(cx).value().trim().is_empty()
        || !app.chat.attachments.is_empty())
        && !app.chat.attachments_loading
        && !invalid_for_model
        && app.current_model().is_some()
        && app.current_conversation().is_some();
    let action = if generating {
        Button::new("composer-stop")
            .danger()
            .bg(cx.theme().danger)
            .rounded(px(17.0))
            .tooltip("Stop generating")
            .size(px(34.0))
            .p_0()
            .absolute()
            .right(px(7.0))
            .bottom(px(7.0))
            .child(render_icon(AppIcon::Stop, IconTone::OnAccent, 16.0, cx))
            .on_click(cx.listener(|this, _, _, cx| this.stop_current_generation(cx)))
            .into_any_element()
    } else {
        Button::new("composer-send")
            .primary()
            .rounded(px(17.0))
            .tooltip("Send message")
            .disabled(!can_send)
            .size(px(34.0))
            .p_0()
            .absolute()
            .right(px(7.0))
            .bottom(px(7.0))
            .child(render_icon(
                AppIcon::ArrowUp,
                if can_send {
                    IconTone::OnAccent
                } else {
                    IconTone::Muted
                },
                20.0,
                cx,
            ))
            .on_click(cx.listener(|this, _, window, cx| this.send_composer(window, cx)))
            .into_any_element()
    };

    let expand = multiline.then(|| {
        Button::new("composer-expand")
            .ghost()
            .rounded(px(17.0))
            .tooltip(if expanded {
                "Collapse input"
            } else {
                "Expand input"
            })
            .size(px(34.0))
            .p_0()
            .absolute()
            .right(px(49.0))
            .bottom(px(7.0))
            .child(render_icon(
                if expanded {
                    AppIcon::Minimize
                } else {
                    AppIcon::Maximize
                },
                IconTone::Muted,
                18.0,
                cx,
            ))
            .on_click(cx.listener(|this, _, window, cx| {
                this.chat
                    .composer_expanded
                    .set(!this.chat.composer_expanded.get());
                let composer = this.chat.composer.clone();
                let selection = composer.read(cx).selected_range();
                composer.update(cx, |composer, cx| composer.focus(window, cx));
                cx.on_next_frame(window, move |_, window, cx| {
                    cx.on_next_frame(window, move |_, window, cx| {
                        composer.update(cx, |composer, cx| {
                            composer.set_selected_range(selection, cx);
                            composer.focus(window, cx);
                        });
                    });
                });
                cx.notify();
            }))
    });

    let add_disabled = generating || app.chat.attachments_loading;
    let add = Button::new("composer-add-attachment")
        .ghost()
        .rounded(px(17.0))
        .tooltip(if app.chat.attachments_loading {
            "Loading attachments"
        } else {
            "Add attachments"
        })
        .disabled(add_disabled)
        .size(px(34.0))
        .p_0()
        .absolute()
        .left(px(7.0))
        .bottom(px(7.0))
        .child(render_icon(AppIcon::Plus, IconTone::Muted, 20.0, cx))
        .on_click(cx.listener(|this, _, _, cx| this.add_attachments(cx)));

    let attachments =
        (!app.chat.attachments.is_empty() || app.chat.attachments_loading).then(|| {
            div()
                .id("composer-attachments")
                .w_full()
                .min_w_0()
                .overflow_x_scroll()
                .restrict_scroll_to_axis()
                .px_3()
                .pt_3()
                .pb_1()
                .flex()
                .items_start()
                .gap_3()
                .children(
                    app.chat
                        .attachments
                        .iter()
                        .map(|attachment| render_attachment(app, attachment, cx)),
                )
                .children(
                    app.chat
                        .attachments_loading
                        .then(|| render_loading_attachment(cx)),
                )
        });

    let input = div()
        .relative()
        .min_w_0()
        .flex_1()
        .overflow_hidden()
        .rounded(px(22.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .shadow_md()
        .capture_action(cx.listener(|this, _: &Paste, _, cx| this.paste_composer_image(cx)))
        .capture_action(cx.listener(|this, action: &Enter, window, cx| {
            let send = !action.shift
                && match this.settings().send_message_shortcut {
                    SendMessageShortcut::Enter => !action.secondary,
                    SendMessageShortcut::SecondaryEnter => action.secondary,
                };
            if send {
                cx.stop_propagation();
                this.send_composer(window, cx);
            }
        }))
        .children(attachments)
        .child({
            let input = Input::new(&app.chat.composer)
                .aria_label("Message")
                .appearance(false)
                .text_size(px(typography.body_size))
                .line_height(px(typography.body_line_height));

            if multiline {
                let input = input.px(px(12.0)).py(px(0.0));
                let input = if expanded { input.h(px(480.0)) } else { input };
                div()
                    .w_full()
                    .min_w_0()
                    .pt(px(12.0))
                    .pr(px(4.0))
                    .child(input)
                    .into_any_element()
            } else {
                input
                    .pl(px(56.0))
                    .pr(px(56.0))
                    .py(px(12.0))
                    .into_any_element()
            }
        })
        .children(multiline.then(|| div().h(px(48.0)).flex_none()))
        .child(add)
        .child(action)
        .children(expand);

    div()
        .flex_none()
        .w_full()
        .px_6()
        .pt_2()
        .pb_4()
        .child(
            div()
                .mx_auto()
                .w_full()
                .max_w(px(message_max_width))
                .child(input),
        )
        .into_any_element()
}

fn composer_is_multiline(app: &OneChat, width: f32, cx: &App) -> bool {
    let composer = app.chat.composer.read(cx);
    let value = composer.value();
    if value.is_empty() {
        app.chat.composer_multiline.set(false);
        app.chat.composer_expanded.set(false);
        return false;
    }
    if value.contains('\n') {
        app.chat.composer_multiline.set(true);
        return true;
    }

    let Some((bounds, line_height)) = composer
        .range_to_bounds(&(0..value.len()))
        .zip(composer.line_height())
    else {
        return app.chat.composer_multiline.get();
    };
    let multiline = bounds.size.height > line_height + px(0.5)
        || bounds.size.width > px((width - 112.0).max(0.0));
    app.chat.composer_multiline.set(multiline);
    if !multiline {
        app.chat.composer_expanded.set(false);
    }
    multiline
}

fn render_attachment(
    app: &OneChat,
    attachment: &crate::domain::AttachmentDraft,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let card = match attachment.kind {
        crate::domain::AttachmentKind::Image | crate::domain::AttachmentKind::Pdf => {
            render_visual_attachment(app, attachment, cx)
        }
        crate::domain::AttachmentKind::Text => render_file_attachment(attachment, cx),
    };

    if cx.reduce_motion() {
        card
    } else {
        div()
            .relative()
            .child(card)
            .with_animation(
                SharedString::from(format!("attachment-appear-{}", attachment.id)),
                Animation::new(Duration::from_millis(180)).with_easing(ease_out_quint()),
                |card, delta| {
                    card.opacity(0.65 + delta * 0.35)
                        .top(px(5.0 * (1.0 - delta)))
                },
            )
            .into_any_element()
    }
}

fn render_visual_attachment(
    app: &OneChat,
    attachment: &crate::domain::AttachmentDraft,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let preview = app.chat.attachment_previews.get(&attachment.id).cloned();
    let object_fit = if attachment.kind == crate::domain::AttachmentKind::Pdf {
        ObjectFit::Contain
    } else {
        ObjectFit::Cover
    };
    let detail = if attachment.kind == crate::domain::AttachmentKind::Pdf {
        format!(
            "{} page{}",
            attachment.files.len(),
            if attachment.files.len() == 1 { "" } else { "s" }
        )
    } else {
        "Image".to_string()
    };

    div()
        .relative()
        .w(px(108.0))
        .flex_none()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .size(px(96.0))
                .overflow_hidden()
                .rounded(px(14.0))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().muted)
                .flex()
                .items_center()
                .justify_center()
                .children(preview.map(|preview| {
                    img(preview)
                        .size_full()
                        .object_fit(object_fit)
                        .into_any_element()
                }))
                .children(
                    (!app.chat.attachment_previews.contains_key(&attachment.id))
                        .then(|| render_icon(AppIcon::FileText, IconTone::Muted, 28.0, cx)),
                ),
        )
        .child(
            div()
                .min_w_0()
                .pr_1()
                .text_size(px(11.0))
                .line_height(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .truncate()
                .child(attachment.name.clone()),
        )
        .child(
            div()
                .text_size(px(10.0))
                .line_height(px(12.0))
                .text_color(cx.theme().muted_foreground)
                .child(detail),
        )
        .child(remove_attachment_button(attachment, cx))
        .into_any_element()
}

fn render_file_attachment(
    attachment: &crate::domain::AttachmentDraft,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .relative()
        .w(px(196.0))
        .h(px(68.0))
        .flex_none()
        .rounded(px(14.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .p_2()
        .pr_8()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .size(px(42.0))
                .flex_none()
                .rounded(px(11.0))
                .bg(cx.theme().accent)
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(AppIcon::FileText, IconTone::Accent, 21.0, cx)),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .min_w_0()
                        .text_size(px(12.0))
                        .line_height(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(attachment.name.clone()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .line_height(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child("Text document"),
                ),
        )
        .child(remove_attachment_button(attachment, cx))
        .into_any_element()
}

fn remove_attachment_button(
    attachment: &crate::domain::AttachmentDraft,
    cx: &mut Context<OneChat>,
) -> Button {
    let id = attachment.id.clone();
    Button::new(SharedString::from(format!(
        "remove-attachment-{}",
        attachment.id
    )))
    .ghost()
    .tooltip("Remove attachment")
    .size(px(24.0))
    .p_0()
    .rounded(px(12.0))
    .absolute()
    .top(px(5.0))
    .right(px(5.0))
    .border_1()
    .border_color(cx.theme().border)
    .bg(cx.theme().popover)
    .shadow_sm()
    .child(render_icon(AppIcon::Close, IconTone::Foreground, 12.0, cx))
    .on_click(cx.listener(move |this, _, _, cx| this.remove_attachment(id.clone(), cx)))
}

fn render_loading_attachment(cx: &App) -> AnyElement {
    div()
        .w(px(108.0))
        .flex_none()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .size(px(96.0))
                .rounded(px(14.0))
                .bg(cx.theme().muted)
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(AppIcon::Plus, IconTone::Muted, 22.0, cx)),
        )
        .child(
            div()
                .text_size(px(11.0))
                .line_height(px(14.0))
                .text_color(cx.theme().muted_foreground)
                .child("Preparing…"),
        )
        .into_any_element()
}
