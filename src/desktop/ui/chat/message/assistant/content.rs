use super::*;

pub(super) fn render_output_content(
    app: &OneChat,
    surface: ResponseSurface,
    output_id: &str,
    content: &str,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let presentation = app.response_presentation(surface);
    if let Some(document) = presentation.markdown_for(output_id, content) {
        markdown::render(
            document,
            output_id,
            &presentation.text_selection,
            scale_factor,
            typography,
            markdown::MarkdownBehavior {
                code_block_wrap: app.settings().code_block_wrap,
                horizontal_scrolls: &presentation.horizontal_scrolls,
            },
            cx,
        )
    } else {
        markdown::render_plain(
            content,
            output_id,
            &presentation.text_selection,
            typography,
            cx,
        )
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
