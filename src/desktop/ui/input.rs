use std::ops::Range;

use gpui::{App, AppContext as _, Context, Entity, Pixels, Size, Window};
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
    state.read(cx).base_state().read(cx).selected_range()
}

pub(crate) fn set_textarea_selection(
    state: &Entity<TextareaState>,
    selection: Range<usize>,
    cx: &mut App,
) {
    let input = state.read(cx).base_state().clone();
    input.update(cx, |input, cx| input.set_selected_range(selection, cx));
}

pub(crate) fn textarea_text_size(
    state: &Entity<TextareaState>,
    range: Range<usize>,
    cx: &App,
) -> Option<(Size<Pixels>, Pixels)> {
    let state = state.read(cx);
    let input = state.base_state().read(cx);
    input
        .range_to_bounds(&range)
        .zip(input.line_height())
        .map(|(bounds, line_height)| (bounds.size, line_height))
}
