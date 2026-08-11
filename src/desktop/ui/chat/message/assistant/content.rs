use super::*;

pub(super) fn render_message_content(
    app: &OneChat,
    message: &AssistantResponse,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let waiting = message.content.is_empty()
        && matches!(
            message.status,
            MessageStatus::Pending | MessageStatus::Streaming
        );
    let editor = app.assistant_message_editor(message);
    if let Some(editor) = editor {
        let save_id = message.id.clone();
        let save_on_mouse_down_id = save_id.clone();
        let enter_id = save_id.clone();
        let can_save = app.can_save_assistant_edit(&message.id, cx);
        let card = div()
            .rounded(px(20.0))
            .border_1()
            .border_color(crate::desktop::ui::theme::palette(cx).accent_border)
            .bg(cx.theme().popover)
            .shadow_xs()
            .p_3()
            .capture_action(cx.listener(move |this, action: &Enter, _, cx| {
                if message_edit_submits(this, action) {
                    cx.stop_propagation();
                    this.save_assistant_edit(enter_id.clone(), cx);
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
                    .text_color(cx.theme().muted_foreground)
                    .child(render_icon(AppIcon::Pencil, IconTone::Accent, 14.0, cx))
                    .child(
                        div()
                            .text_size(px(typography.metadata_size))
                            .line_height(px(typography.metadata_line_height))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Editing response"),
                    ),
            )
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_move(|_, _, cx| cx.stop_propagation())
                    .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_up_out(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        Input::new(&editor)
                            .aria_label("Edit assistant response")
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
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(typography.micro_size))
                            .line_height(px(typography.micro_line_height))
                            .text_color(cx.theme().muted_foreground)
                            .child(message_edit_shortcut_hint(app)),
                    )
                    .child(
                        editor_cancel_button(
                            SharedString::from(format!("cancel-edit-message-{}", message.id)),
                            cx,
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                this.cancel_message_edit(cx);
                            }),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_message_edit(cx))),
                    )
                    .child(
                        editor_save_button(
                            SharedString::from(format!("save-edit-message-{}", message.id)),
                            "Save response edit",
                            !can_save,
                            cx,
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.save_assistant_edit(save_on_mouse_down_id.clone(), cx);
                            }),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.save_assistant_edit(save_id.clone(), cx)
                        })),
                    ),
            );
        animated_editor(
            card.into_any_element(),
            SharedString::from(format!("assistant-editor-{}", message.id)),
            cx,
        )
    } else if waiting {
        div()
            .flex()
            .items_center()
            .gap_2()
            .text_size(px(typography.metadata_size))
            .line_height(px(typography.metadata_line_height))
            .text_color(cx.theme().muted_foreground)
            .child(div().size(px(7.0)).rounded_full().bg(cx.theme().primary))
            .child(waiting_label(message))
            .into_any_element()
    } else if let Some(document) = app.markdown_for(&message.id, &message.content) {
        markdown::render(
            document,
            &message.id,
            &app.chat.text_selection,
            scale_factor,
            typography,
            app.settings().code_block_wrap,
            cx,
        )
    } else {
        markdown::render_plain(
            &message.content,
            &message.id,
            &app.chat.text_selection,
            typography,
            cx,
        )
    }
}
