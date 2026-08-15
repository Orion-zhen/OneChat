use super::*;

pub(in crate::desktop::ui::chat) fn render_assistant_opening(
    app: &OneChat,
    message_max_width: f32,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let conversation = app
        .current_conversation()
        .expect("assistant opening requires a conversation");
    let message_id = format!("assistant-opening-{}", conversation.id);
    let action_group: SharedString =
        format!("assistant-opening-actions-{}", conversation.id).into();
    let content =
        if let Some(document) = app.markdown_for(&message_id, &conversation.assistant_opening) {
            markdown::render(
                document,
                &message_id,
                &app.chat.text_selection,
                scale_factor,
                typography,
                markdown::MarkdownBehavior {
                    code_block_wrap: app.settings().code_block_wrap,
                    horizontal_scrolls: &app.chat.horizontal_scrolls,
                },
                cx,
            )
        } else {
            markdown::render_plain(
                &conversation.assistant_opening,
                &message_id,
                &app.chat.text_selection,
                typography,
                cx,
            )
        };
    let edit_id = format!("edit-{message_id}");
    let message = div()
        .id(SharedString::from(format!(
            "assistant-opening-{}",
            conversation.id
        )))
        .mx_auto()
        .group(action_group.clone())
        .mb_8()
        .w_full()
        .max_w(px(message_max_width))
        .child(
            div()
                .mb_3()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .size(px(24.0))
                        .flex_none()
                        .rounded_lg()
                        .bg(cx.theme().accent)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(cx.theme().primary)
                        .child(render_icon(AppIcon::Sparkles, IconTone::Accent, 13.0, cx)),
                )
                .child(
                    div()
                        .text_size(px(typography.metadata_size))
                        .line_height(px(typography.metadata_line_height))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child("Assistant · Opening"),
                ),
        )
        .child(content)
        .child(
            div()
                .mt_3()
                .min_h(px(24.0))
                .invisible()
                .group_hover(action_group, |actions| actions.visible())
                .flex()
                .items_center()
                .gap_1()
                .child(CopyButton::new(
                    SharedString::from(format!("copy-{message_id}")),
                    conversation.assistant_opening.clone(),
                ))
                .child(
                    icon_button(edit_id, AppIcon::Pencil, IconTone::Muted, cx).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.begin_edit_assistant_opening(window, cx)
                        }),
                    ),
                ),
        )
        .into_any_element();
    animated_message(message, message_id)
}
