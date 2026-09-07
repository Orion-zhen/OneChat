use gpui::{AnyElement, Context, Window, div, prelude::*};

use super::{prompts, result, source};
use crate::desktop::{
    app::OneChat,
    ui::{layout::LayoutClass, theme},
};

pub(crate) fn render(
    app: &mut OneChat,
    available_width: f32,
    scale_factor: f32,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    app.translation.sync_prompt_controls(window, cx);
    app.translation
        .output
        .presentation
        .text_selection
        .begin_frame(app.translation.output.scroll.clone());
    let layout = LayoutClass::from_width(available_width);
    let stacked = !layout.is_wide();
    let workbench = div()
        .min_w_0()
        .when(stacked, |workbench| {
            workbench.flex_none().flex().flex_col().gap_3()
        })
        .when(!stacked, |workbench| {
            workbench.min_h_0().flex_1().flex().gap_3()
        })
        .child(source::render(app, layout, cx))
        .child(result::render(app, layout, scale_factor, cx));

    div()
        .id("translation-page-scroll")
        .relative()
        .size_full()
        .min_w_0()
        .bg(theme::palette(cx).canvas)
        .when(stacked, |page| page.overflow_y_scroll())
        .child(
            div()
                .size_full()
                .min_w_0()
                .p_4()
                .flex()
                .flex_col()
                .gap_2()
                .child(workbench)
                .child(prompts::render(app, cx)),
        )
        .into_any_element()
}
