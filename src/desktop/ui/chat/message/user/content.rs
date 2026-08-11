use super::edit_attachments::{
    render_edit_attachment_loading, render_edit_draft_attachment, render_edit_stored_attachment,
};
use super::*;

pub(super) fn render_message_content(
    app: &OneChat,
    turn: &Turn,
    user_message_max_width: f32,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let editor = app.user_message_editor(turn);
    if let Some(editor) = editor {
        let save_id = turn.id.clone();
        let save_on_mouse_down_id = save_id.clone();
        let enter_id = save_id.clone();
        let add_id = turn.id.clone();
        let editor_input = editor.input.clone();
        let attachment_count = editor.attachments.len() + editor.attachment_drafts.len();
        let attachments_loading = editor.attachment_load_id.is_some();
        let has_attachments = attachment_count > 0 || attachments_loading;
        let can_save = app.can_save_user_edit(&turn.id, cx);
        let width = if has_attachments {
            user_message_max_width
        } else {
            user_editor_width(
                &editor_input.read(cx).value(),
                user_message_max_width,
                typography.body_size,
            )
        };
        let palette = crate::desktop::ui::theme::palette(cx).user_message;
        let card = div()
            .w(px(width))
            .rounded(px(20.0))
            .border_1()
            .border_color(crate::desktop::ui::theme::palette(cx).accent_border)
            .bg(palette.background)
            .shadow_xs()
            .p_3()
            .capture_action(cx.listener(move |this, action: &Enter, _, cx| {
                if message_edit_submits(this, action) {
                    cx.stop_propagation();
                    this.save_user_edit(enter_id.clone(), cx);
                }
            }))
            .capture_action(cx.listener(|this, _: &InputEscape, _, cx| {
                cx.stop_propagation();
                this.cancel_message_edit(cx);
            }))
            .child(
                div()
                    .pb_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_color(palette.muted_foreground)
                    .child(render_icon(AppIcon::Pencil, IconTone::Accent, 14.0, cx))
                    .child(
                        div()
                            .text_size(px(typography.metadata_size))
                            .line_height(px(typography.metadata_line_height))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Editing message"),
                    ),
            )
            .children(has_attachments.then(|| {
                div()
                    .id(SharedString::from(format!(
                        "edit-user-attachments-{}",
                        turn.id
                    )))
                    .w_full()
                    .min_w_0()
                    .overflow_x_scroll()
                    .restrict_scroll_to_axis()
                    .pb_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .children(editor.attachments.iter().map(|attachment| {
                        render_edit_stored_attachment(app, &turn.id, attachment, cx)
                    }))
                    .children(editor.attachment_drafts.iter().map(|attachment| {
                        render_edit_draft_attachment(
                            app,
                            &turn.id,
                            attachment,
                            editor.attachment_previews.get(&attachment.id).cloned(),
                            cx,
                        )
                    }))
                    .children(attachments_loading.then(|| render_edit_attachment_loading(cx)))
            }))
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_move(|_, _, cx| cx.stop_propagation())
                    .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_up_out(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        Input::new(&editor_input)
                            .aria_label("Edit user message")
                            .appearance(false)
                            .w_full()
                            .px(px(2.0))
                            .py(px(4.0))
                            .text_size(px(typography.body_size))
                            .line_height(px(typography.body_line_height)),
                    ),
            )
            .child(
                div()
                    .pt_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        Button::new(SharedString::from(format!(
                            "add-edit-user-attachment-{}",
                            turn.id
                        )))
                        .ghost()
                        .rounded(px(17.0))
                        .tooltip("Add attachment")
                        .size(px(34.0))
                        .p_0()
                        .disabled(
                            attachments_loading
                                || attachment_count
                                    >= crate::application::attachments::MAX_ATTACHMENTS,
                        )
                        .child(render_icon(AppIcon::Plus, IconTone::Muted, 18.0, cx))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.add_message_edit_attachments(add_id.clone(), cx)
                        })),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(typography.micro_size))
                                    .line_height(px(typography.micro_line_height))
                                    .text_color(palette.muted_foreground)
                                    .child(message_edit_shortcut_hint(app)),
                            )
                            .child(
                                editor_cancel_button(
                                    SharedString::from(format!("cancel-edit-user-{}", turn.id)),
                                    cx,
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.cancel_message_edit(cx);
                                    }),
                                )
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.cancel_message_edit(cx)),
                                ),
                            )
                            .child(
                                editor_save_button(
                                    SharedString::from(format!("save-edit-user-{}", turn.id)),
                                    "Save edit and regenerate response",
                                    !can_save,
                                    cx,
                                )
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.save_user_edit(save_on_mouse_down_id.clone(), cx);
                                    }),
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| this.save_user_edit(save_id.clone(), cx),
                                )),
                            ),
                    ),
            );
        animated_editor(
            card.into_any_element(),
            SharedString::from(format!("user-editor-{}", turn.id)),
            cx,
        )
    } else {
        let palette = crate::desktop::ui::theme::palette(cx).user_message;
        div()
            .max_w(px(user_message_max_width))
            .min_w_0()
            .flex()
            .flex_col()
            .items_end()
            .gap_2()
            .children(turn.user.attachments.iter().map(|attachment| {
                render_sent_attachment(app, attachment, user_message_max_width, cx)
            }))
            .children((!turn.user.content.is_empty()).then(|| {
                div()
                    .max_w(px(user_message_max_width))
                    .rounded(px(18.0))
                    .border_1()
                    .border_color(palette.border)
                    .bg(palette.background)
                    .px_4()
                    .py_3()
                    .text_color(palette.foreground)
                    .whitespace_normal()
                    .text_size(px(typography.body_size))
                    .line_height(px(typography.body_line_height))
                    .child(
                        if let Some(document) = app.markdown_for(&turn.user.id, &turn.user.content)
                        {
                            markdown::render_user(
                                document,
                                &turn.user.id,
                                &app.chat.text_selection,
                                scale_factor,
                                typography,
                                app.settings().code_block_wrap,
                                cx,
                            )
                        } else {
                            markdown::render_user_plain(
                                &turn.user.content,
                                &turn.user.id,
                                &app.chat.text_selection,
                                typography,
                                cx,
                            )
                        },
                    )
            }))
            .into_any_element()
    }
}
