use gpui::{AnyElement, App, Image, ImageFormat, div, img, prelude::*, px};
use gpui_component::{ActiveTheme as _, ThemeMode};

use super::{InlineMetrics, element_key};
use crate::{
    desktop::ui::theme,
    markdown::{Formula, render_formula_cached},
};

pub(super) fn render_formula_element(
    formula: &Formula,
    scale_factor: f32,
    metrics: InlineMetrics,
    cx: &App,
) -> AnyElement {
    match render_formula_cached(
        &formula.source,
        formula.display,
        cx.theme().mode == ThemeMode::Dark,
        scale_factor,
        metrics.formula_scale(),
    ) {
        Ok(rendered) => {
            let image =
                std::sync::Arc::new(Image::from_bytes(ImageFormat::Svg, rendered.svg.clone()));
            div()
                .id(element_key("formula", &formula.source))
                .when(formula.display, |element| {
                    element.w_full().justify_center().overflow_scroll().py_2()
                })
                .when(!formula.display, |element| element.px_1())
                .flex()
                .items_center()
                .child(
                    img(image)
                        .w(px(rendered.width))
                        .h(px(rendered.height))
                        .flex_none(),
                )
                .into_any_element()
        }
        Err(error) => div()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().danger)
            .bg(cx.theme().muted)
            .px_2()
            .py_1()
            .font(theme::code_font(cx))
            .text_size(px(metrics.code_size()))
            .line_height(px(metrics.code_line_height()))
            .text_color(cx.theme().danger)
            .child(format!("{} · {error}", formula.source))
            .into_any_element(),
    }
}
