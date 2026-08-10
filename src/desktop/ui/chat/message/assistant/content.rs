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
        div()
            .rounded_xl()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .p_3()
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_move(|_, _, cx| cx.stop_propagation())
                    .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_up_out(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_action(
                        cx.listener(|this, _: &InputEscape, _, cx| this.cancel_message_edit(cx)),
                    )
                    .child(
                        Input::new(&editor)
                            .aria_label("Edit assistant response")
                            .bg(cx.theme().muted)
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
                        large_icon_button(
                            SharedString::from(format!("cancel-edit-message-{}", message.id)),
                            AppIcon::Close,
                            IconTone::Muted,
                            cx,
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.cancel_message_edit(cx);
                                cx.stop_propagation();
                            }),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_message_edit(cx))),
                    )
                    .child(
                        primary_icon_button(
                            SharedString::from(format!("save-edit-message-{}", message.id)),
                            AppIcon::Save,
                            cx,
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.save_assistant_edit(save_on_mouse_down_id.clone(), cx);
                                cx.stop_propagation();
                            }),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.save_assistant_edit(save_id.clone(), cx)
                        })),
                    ),
            )
            .into_any_element()
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
