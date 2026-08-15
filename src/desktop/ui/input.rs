use std::ops::Range;

use gpui::{App, AppContext as _, Context, Entity, Window};
use gpui_component::input::{InputState, TextareaState};

use crate::desktop::app::OneChat;

pub(crate) fn single_line(
    value: impl Into<String>,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(value.into())
            .placeholder(placeholder)
    })
}

pub(crate) fn masked(
    value: impl Into<String>,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(value.into())
            .placeholder(placeholder)
            .masked(true)
    })
}

pub(crate) fn textarea(
    value: impl Into<String>,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> Entity<TextareaState> {
    cx.new(|cx| {
        TextareaState::new(window, cx)
            .soft_wrap(true)
            .default_value(value.into())
            .placeholder(placeholder)
    })
}

pub(crate) fn textarea_selection(state: &Entity<TextareaState>, cx: &App) -> Range<usize> {
    state.read(cx).selected_range()
}

pub(crate) fn set_textarea_selection(
    state: &Entity<TextareaState>,
    selection: Range<usize>,
    cx: &mut App,
) {
    state.update(cx, |state, cx| state.set_selected_range(selection, cx));
}
