use gpui::{AnyElement, Context, div, prelude::*};

use super::{prompts, result, source};
use crate::desktop::{
    app::OneChat,
    ui::{layout::LayoutClass, theme},
};

pub(crate) fn render(
    app: &OneChat,
    available_width: f32,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
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
