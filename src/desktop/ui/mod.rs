mod badges;
pub(crate) mod chat;
pub(crate) mod controls;
pub(crate) mod copy_button;
pub(crate) mod icons;
pub(crate) mod input;
pub(crate) mod inspector;
pub(crate) mod markdown;
mod mcp;
mod model;
pub(crate) mod motion;
pub(crate) mod playback;
pub(crate) mod selectable_text;
pub(crate) mod settings;
pub(crate) mod shell;
pub(crate) mod stream;
mod text;
pub(crate) mod theme;
pub(crate) mod tts;
pub(crate) mod typography;

use std::borrow::Cow;

use gpui::{App, Div, IntoElement, div, prelude::*, px};
use gpui_component::ActiveTheme as _;
use lucide_icons::LUCIDE_FONT_BYTES;

pub(crate) const SIDEBAR_WIDTH: f32 = 260.0;

pub(crate) fn spaced_select_item(content: impl IntoElement, cx: &App) -> Div {
    // gpui-component paints hover/selection on the outer row and does not expose its margins.
    // Cover the row's top edge, including its padding and check column, to keep highlights apart.
    div().relative().w_full().child(content).child(
        div()
            .absolute()
            .top(px(-4.0))
            .left(px(-8.0))
            .right(px(-28.0))
            .h(px(2.0))
            .bg(cx.theme().popover),
    )
}

pub(crate) fn init(cx: &mut App) {
    gpui_component::init(cx);
    cx.text_system()
        .add_fonts(vec![Cow::Borrowed(LUCIDE_FONT_BYTES)])
        .expect("failed to register Lucide icon font");
    theme::init(cx);
    shell::init(cx);
}
