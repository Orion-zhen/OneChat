use super::*;

pub(super) fn render_edit_stored_attachment(
    app: &OneChat,
    turn_id: &str,
    attachment: &crate::domain::Attachment,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    if attachment.kind == crate::domain::AttachmentKind::Audio {
        return render_audio_attachment_card(
            app,
            &attachment.id,
            &attachment.name,
            attachment.audio.as_ref(),
            240.0,
            Some(edit_remove_button(turn_id, &attachment.id, cx).into_any_element()),
            cx,
        );
    }
    let visual = match attachment.kind {
        crate::domain::AttachmentKind::Text | crate::domain::AttachmentKind::Document => {
            edit_attachment_icon(cx)
        }
        crate::domain::AttachmentKind::Audio => unreachable!(),
        crate::domain::AttachmentKind::Image | crate::domain::AttachmentKind::Pdf => attachment
            .files
            .first()
            .and_then(|file| app.attachment_file_path(file))
            .map(|path| {
                img(path)
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            })
            .unwrap_or_else(|| edit_attachment_icon(cx)),
    };
    edit_attachment_card(
        turn_id,
        &attachment.id,
        &attachment.name,
        attachment_detail(
            &attachment.name,
            attachment.kind,
            attachment.files.iter().map(|file| file.kind),
            attachment.audio.as_ref(),
        ),
        visual,
        cx,
    )
}

pub(super) fn render_edit_draft_attachment(
    app: &OneChat,
    turn_id: &str,
    attachment: &crate::domain::AttachmentDraft,
    preview: Option<std::sync::Arc<gpui::Image>>,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    if attachment.kind == crate::domain::AttachmentKind::Audio {
        return render_audio_attachment_card(
            app,
            &attachment.id,
            &attachment.name,
            attachment.audio.as_ref(),
            240.0,
            Some(edit_remove_button(turn_id, &attachment.id, cx).into_any_element()),
            cx,
        );
    }
    let visual = preview
        .map(|preview| {
            img(preview)
                .size_full()
                .object_fit(ObjectFit::Contain)
                .into_any_element()
        })
        .unwrap_or_else(|| edit_attachment_icon(cx));
    edit_attachment_card(
        turn_id,
        &attachment.id,
        &attachment.name,
        attachment_detail(
            &attachment.name,
            attachment.kind,
            attachment.files.iter().map(|file| file.kind),
            attachment.audio.as_ref(),
        ),
        visual,
        cx,
    )
}

fn edit_attachment_icon(cx: &App) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(render_icon(AppIcon::FileText, IconTone::Accent, 21.0, cx))
        .into_any_element()
}

fn edit_attachment_card(
    turn_id: &str,
    attachment_id: &str,
    name: &str,
    detail: String,
    visual: AnyElement,
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
                .size(px(44.0))
                .flex_none()
                .overflow_hidden()
                .rounded(px(11.0))
                .bg(cx.theme().accent)
                .child(visual),
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
                        .child(name.to_string()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .line_height(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(detail),
                ),
        )
        .child(edit_remove_button(turn_id, attachment_id, cx))
        .into_any_element()
}

fn edit_remove_button(turn_id: &str, attachment_id: &str, cx: &mut Context<OneChat>) -> Button {
    let remove_turn_id = turn_id.to_string();
    let remove_attachment_id = attachment_id.to_string();
    Button::new(SharedString::from(format!(
        "remove-edit-attachment-{turn_id}-{attachment_id}"
    )))
    .ghost()
    .tooltip("Remove attachment")
    .size(px(24.0))
    .p_0()
    .rounded(px(12.0))
    .absolute()
    .top(px(5.0))
    .right(px(5.0))
    .child(render_icon(AppIcon::Close, IconTone::Foreground, 12.0, cx))
    .on_click(cx.listener(move |this, _, _, cx| {
        this.remove_message_edit_attachment(
            remove_turn_id.clone(),
            remove_attachment_id.clone(),
            cx,
        )
    }))
}

pub(super) fn render_edit_attachment_loading(cx: &App) -> AnyElement {
    div()
        .w(px(108.0))
        .h(px(68.0))
        .flex_none()
        .rounded(px(14.0))
        .bg(cx.theme().muted)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(cx.theme().muted_foreground)
        .child("Preparing…")
        .into_any_element()
}
