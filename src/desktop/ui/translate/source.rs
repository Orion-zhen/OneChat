use gpui::{AnyElement, Context, div, prelude::*, px};
use gpui_component::{ActiveTheme as _, input::Textarea};

use super::components::{header_controls, language_select_slot, panel, panel_header};
use crate::desktop::{
    app::OneChat,
    ui::{controls::select_control, icons::AppIcon, layout::LayoutClass},
};

pub(super) fn render(app: &OneChat, layout: LayoutClass, cx: &mut Context<OneChat>) -> AnyElement {
    let stacked = !layout.is_wide();
    let narrow = layout.is_narrow();
    let source = app.translation.controls.source.read(cx).value();
    let char_count = source.chars().count();
    let has_status = char_count > 0 || app.translation.error.is_some();

    panel(stacked, cx)
        .child(
            panel_header("Source", AppIcon::FileText, narrow, cx).child(
                header_controls(narrow).child(
                    language_select_slot(narrow).child(
                        select_control(&app.translation.controls.source_language)
                            .w_full()
                            .disabled(app.translation.is_generating()),
                    ),
                ),
            ),
        )
        .child(
            div()
                .min_h(px(if stacked { 240.0 } else { 0.0 }))
                .min_w_0()
                .flex_1()
                .p_4()
                .child(
                    Textarea::new(&app.translation.controls.source)
                        .appearance(false)
                        .w_full()
                        .h_full()
                        .text_size(px(15.0))
                        .line_height(px(23.0))
                        .aria_label("Text to translate"),
                ),
        )
        .children(has_status.then(|| {
            div()
                .flex_none()
                .min_h(px(30.0))
                .px_4()
                .pb_3()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .children(app.translation.error.as_ref().map(|error| {
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_color(cx.theme().danger)
                        .child(error.clone())
                }))
                .children(
                    (char_count > 0)
                        .then(|| div().flex_none().child(format!("{char_count} characters"))),
                )
        }))
        .into_any_element()
}
