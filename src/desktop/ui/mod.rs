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
mod segmented_control;
pub(crate) mod selectable_text;
pub(crate) mod settings;
pub(crate) mod shell;
pub(crate) mod stream;
mod text;
pub(crate) mod theme;
pub(crate) mod tts;
pub(crate) mod typography;

use std::borrow::Cow;

use gpui::App;
use lucide_icons::LUCIDE_FONT_BYTES;

pub(crate) use segmented_control::SegmentedControl;

pub(crate) const SIDEBAR_WIDTH: f32 = 260.0;

pub(crate) fn init(cx: &mut App) {
    gpui_component::init(cx);
    cx.text_system()
        .add_fonts(vec![Cow::Borrowed(LUCIDE_FONT_BYTES)])
        .expect("failed to register Lucide icon font");
    theme::init(cx);
    shell::init(cx);
}
