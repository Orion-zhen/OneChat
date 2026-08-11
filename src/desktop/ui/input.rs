use gpui::{AppContext as _, Context, Entity, Window};
use gpui_component::input::InputState;

use crate::desktop::app::OneChat;

pub(crate) fn multiline(
    value: impl Into<String>,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .multi_line(true)
            .soft_wrap(true)
            .default_value(value.into())
            .placeholder(placeholder)
    })
}
