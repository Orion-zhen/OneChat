use super::*;

pub(super) fn render_attachments(app: &OneChat, cx: &mut Context<OneChat>) -> Option<AnyElement> {
    (!app.chat.attachments.is_empty() || app.chat.attachments_loading).then(|| {
        let attachment_scroll = app.chat.horizontal_scrolls.handle("composer-attachments");
        let boundary_scroll = attachment_scroll.clone();

        let attachments = div()
            .id("composer-attachments")
            .w_full()
            .min_w_0()
            .track_scroll(&attachment_scroll)
            .on_scroll_wheel(move |event, _, cx| {
                if nested_horizontal_scroll_captures(event, &boundary_scroll) {
                    cx.stop_propagation();
                }
            })
            .overflow_x_scroll()
            .restrict_scroll_to_axis()
            .px_3()
            .pt_3()
            .pb_5()
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
            );

        div()
            .relative()
            .w_full()
            .min_w_0()
            .child(attachments)
            .child(always_horizontal_scrollbar(
                "composer-attachments-scrollbar",
                &attachment_scroll,
                cx,
            ))
            .into_any_element()
    })
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
        crate::domain::AttachmentKind::Audio => render_audio_draft_attachment(app, attachment, cx),
        crate::domain::AttachmentKind::Text | crate::domain::AttachmentKind::Document => {
            render_file_attachment(attachment, cx)
        }
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
    let detail = attachment_detail(
        &attachment.name,
        attachment.kind,
        attachment.files.iter().map(|file| file.kind),
        attachment.audio.as_ref(),
    );

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
        .child(
            div()
                .absolute()
                .top(px(5.0))
                .right(px(5.0))
                .child(remove_attachment_button(attachment, cx)),
        )
        .into_any_element()
}

fn render_audio_draft_attachment(
    app: &OneChat,
    attachment: &crate::domain::AttachmentDraft,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let remove = remove_attachment_button(attachment, cx).into_any_element();
    render_audio_attachment_card(
        app,
        &attachment.id,
        &attachment.name,
        attachment.audio.as_ref(),
        240.0,
        Some(remove),
        cx,
    )
}

fn render_file_attachment(
    attachment: &crate::domain::AttachmentDraft,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let detail = attachment_detail(
        &attachment.name,
        attachment.kind,
        attachment.files.iter().map(|file| file.kind),
        attachment.audio.as_ref(),
    );
    div()
        .w(px(196.0))
        .h(px(68.0))
        .flex_none()
        .rounded(px(14.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .p(px(5.0))
        .flex()
        .items_start()
        .gap(px(3.0))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .h_full()
                .pl(px(3.0))
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
                                .child(detail),
                        ),
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
    .flex_none()
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
