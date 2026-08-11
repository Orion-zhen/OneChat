use super::*;

pub(super) fn user_editor_width(content: &str, max_width: f32, font_size: f32) -> f32 {
    let text_scale = font_size / USER_EDITOR_MEASUREMENT_FONT_SIZE;
    let text_width = content
        .lines()
        .map(|line| {
            line.graphemes(true)
                .map(|grapheme| (if grapheme.is_ascii() { 8.0 } else { 15.0 }) * text_scale)
                .sum::<f32>()
        })
        .fold(0.0, f32::max);
    (text_width + USER_EDITOR_HORIZONTAL_CHROME)
        .max(USER_EDITOR_MIN_WIDTH)
        .min(max_width)
}

pub(super) fn render_sent_attachment(
    app: &OneChat,
    attachment: &crate::domain::Attachment,
    max_width: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    match attachment.kind {
        crate::domain::AttachmentKind::Image => render_sent_image(app, attachment, max_width, cx),
        crate::domain::AttachmentKind::Audio => render_audio_attachment_card(
            app,
            &attachment.id,
            &attachment.name,
            attachment.audio.as_ref(),
            300.0_f32.min(max_width),
            None,
            cx,
        ),
        crate::domain::AttachmentKind::Text
        | crate::domain::AttachmentKind::Pdf
        | crate::domain::AttachmentKind::Document => {
            render_sent_file(app, attachment, max_width, cx)
        }
    }
}

fn sent_image_size(path: &std::path::Path, max_width: f32) -> (f32, f32) {
    static DIMENSIONS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, (u32, u32)>>,
    > = std::sync::OnceLock::new();

    let dimensions = DIMENSIONS.get_or_init(Default::default);
    let (source_width, source_height) = *dimensions
        .lock()
        .expect("attachment dimension cache poisoned")
        .entry(path.to_path_buf())
        .or_insert_with(|| image::image_dimensions(path).unwrap_or((320, 200)));
    let source_width = source_width.max(1) as f32;
    let source_height = source_height.max(1) as f32;
    let scale = (max_width.min(SENT_IMAGE_MAX_WIDTH) / source_width)
        .min(SENT_IMAGE_MAX_HEIGHT / source_height)
        .min(1.0);
    (source_width * scale, source_height * scale)
}

fn render_sent_image(
    app: &OneChat,
    attachment: &crate::domain::Attachment,
    max_width: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let Some(path) = attachment
        .files
        .first()
        .and_then(|file| app.attachment_file_path(file))
    else {
        return sent_attachment_fallback(attachment, max_width, cx);
    };
    let (width, height) = sent_image_size(&path, max_width);
    let fallback_name = attachment.name.clone();
    let muted = cx.theme().muted;
    let muted_foreground = cx.theme().muted_foreground;

    div()
        .id(SharedString::from(format!(
            "user-attachment-image-{}",
            attachment.id
        )))
        .w(px(width))
        .h(px(height))
        .flex_none()
        .overflow_hidden()
        .rounded(px(16.0))
        .border_1()
        .border_color(crate::desktop::ui::theme::palette(cx).media_border)
        .bg(muted)
        .shadow_xs()
        .child(
            img(path)
                .size_full()
                .rounded(px(15.0))
                .object_fit(ObjectFit::Contain)
                .with_fallback(move || {
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.0))
                        .text_color(muted_foreground)
                        .child(format!("Could not preview {fallback_name}"))
                        .into_any_element()
                }),
        )
        .into_any_element()
}

fn attachment_icon(cx: &App) -> AnyElement {
    div()
        .size(px(44.0))
        .flex_none()
        .rounded(px(12.0))
        .bg(cx.theme().accent)
        .flex()
        .items_center()
        .justify_center()
        .child(render_icon(AppIcon::FileText, IconTone::Accent, 21.0, cx))
        .into_any_element()
}

fn sent_file_card(
    attachment: &crate::domain::Attachment,
    detail: String,
    visual: AnyElement,
    max_width: f32,
    cx: &App,
) -> AnyElement {
    div()
        .w(px(260.0_f32.min(max_width)))
        .min_h(px(68.0))
        .rounded(px(16.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .shadow_xs()
        .p_2()
        .flex()
        .items_center()
        .gap_3()
        .text_color(cx.theme().foreground)
        .child(visual)
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
        )
        .into_any_element()
}

fn render_sent_file(
    app: &OneChat,
    attachment: &crate::domain::Attachment,
    max_width: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let is_pdf = attachment.kind == crate::domain::AttachmentKind::Pdf;
    let detail = attachment_detail(
        &attachment.name,
        attachment.kind,
        attachment.files.iter().map(|file| file.kind),
        attachment.audio.as_ref(),
    );
    let visual = if is_pdf {
        attachment
            .files
            .first()
            .and_then(|file| app.attachment_file_path(file))
            .map(|path| {
                div()
                    .w(px(42.0))
                    .h(px(52.0))
                    .flex_none()
                    .overflow_hidden()
                    .rounded(px(9.0))
                    .border_1()
                    .border_color(crate::desktop::ui::theme::palette(cx).document_border)
                    .bg(crate::desktop::ui::theme::palette(cx).document_background)
                    .shadow_xs()
                    .child(img(path).size_full().object_fit(ObjectFit::Contain))
                    .into_any_element()
            })
            .unwrap_or_else(|| attachment_icon(cx))
    } else {
        attachment_icon(cx)
    };

    sent_file_card(attachment, detail, visual, max_width, cx)
}

fn sent_attachment_fallback(
    attachment: &crate::domain::Attachment,
    max_width: f32,
    cx: &App,
) -> AnyElement {
    sent_file_card(
        attachment,
        "Attachment unavailable".into(),
        attachment_icon(cx),
        max_width,
        cx,
    )
}
