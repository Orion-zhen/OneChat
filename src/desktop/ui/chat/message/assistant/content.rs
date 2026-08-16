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
    let editor = app.assistant_output_editor(message, &message.id);
    if let Some(output) = editor {
        div()
            .mb_4()
            .child(render_output_editor(
                &message.id,
                &output.input,
                0,
                1,
                typography,
                cx,
            ))
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
            markdown::MarkdownBehavior {
                code_block_wrap: app.settings().code_block_wrap,
                horizontal_scrolls: &app.chat.horizontal_scrolls,
            },
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

pub(super) fn render_output_content(
    app: &OneChat,
    output_id: &str,
    content: &str,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    if let Some(document) = app.markdown_for(output_id, content) {
        markdown::render(
            document,
            output_id,
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
        markdown::render_plain(content, output_id, &app.chat.text_selection, typography, cx)
    }
}

pub(super) fn render_output_editor(
    output_id: &str,
    editor: &gpui::Entity<gpui_component::input::TextareaState>,
    index: usize,
    count: usize,
    typography: MessageTypography,
    cx: &App,
) -> AnyElement {
    render_assistant_text_editor(
        output_id,
        editor,
        if count == 1 {
            "Editing output".to_string()
        } else {
            format!("Editing output {} of {count}", index + 1)
        },
        format!("Edit assistant output {}", index + 1),
        typography,
        cx,
    )
}
