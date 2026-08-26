use std::{cell::RefCell, collections::HashMap};

use gpui::{
    AnyElement, App, ElementId, ScrollDelta, ScrollHandle, ScrollWheelEvent, prelude::*, px,
};
use gpui_component::scroll::{Scrollbar, ScrollbarMode};

use crate::desktop::{
    branch_swipe::{horizontal_delta_dominates, should_capture_nested_horizontal_scroll},
    ui::theme,
};

pub fn follow_after_scroll(current: bool, delta_y: f32, distance_from_bottom: f32) -> bool {
    if delta_y > 0.0 {
        false
    } else if delta_y < 0.0 {
        current || distance_from_bottom + delta_y <= 48.0
    } else {
        current
    }
}

pub(crate) fn should_capture_nested_scroll(delta_y: f32, offset_y: f32, max_offset_y: f32) -> bool {
    if delta_y > 0.0 {
        offset_y < 0.0
    } else if delta_y < 0.0 {
        max_offset_y + offset_y > 0.0
    } else {
        false
    }
}

#[derive(Default)]
pub(crate) struct HorizontalScrollRegistry {
    handles: RefCell<HashMap<String, ScrollHandle>>,
}

impl HorizontalScrollRegistry {
    pub(crate) fn handle(&self, key: impl Into<String>) -> ScrollHandle {
        self.handles
            .borrow_mut()
            .entry(key.into())
            .or_default()
            .clone()
    }

    pub(crate) fn clear(&self) {
        self.handles.borrow_mut().clear();
    }
}

fn horizontal_scrollbar(id: impl Into<ElementId>, scroll: &ScrollHandle, cx: &App) -> Scrollbar {
    let palette = theme::palette(cx);
    Scrollbar::horizontal(scroll)
        .id(id)
        .mode(ScrollbarMode::Always)
        .styles(|styles| {
            styles
                .track(|style| style.bg(gpui::transparent_black()))
                .track_hover(|style| style.bg(gpui::transparent_black()))
                .track_active(|style| {
                    style
                        .bg(gpui::transparent_black())
                        .border_color(gpui::transparent_black())
                })
                .thumb(|style| style.bg(palette.scrollbar_thumb))
                .thumb_hover(|style| style.bg(palette.scrollbar_thumb_hover))
                .thumb_active(|style| style.bg(palette.scrollbar_thumb_active))
        })
}

pub(crate) fn always_horizontal_scrollbar(
    id: impl Into<ElementId>,
    scroll: &ScrollHandle,
    cx: &App,
) -> AnyElement {
    horizontal_scrollbar(id, scroll, cx).into_any_element()
}

pub(crate) fn compact_horizontal_scrollbar(
    id: impl Into<ElementId>,
    scroll: &ScrollHandle,
    cx: &App,
) -> AnyElement {
    horizontal_scrollbar(id, scroll, cx)
        .styles(|styles| {
            styles
                .track(|style| style.width(px(8.0)))
                .thumb(|style| style.width(px(4.0)).inset(px(2.0)).radius(px(2.0)))
        })
        .into_any_element()
}

pub(crate) fn nested_horizontal_scroll_captures(
    event: &ScrollWheelEvent,
    scroll: &ScrollHandle,
) -> bool {
    let ScrollDelta::Pixels(delta) = event.delta else {
        return false;
    };
    let delta_x = f32::from(delta.x);
    let delta_y = f32::from(delta.y);
    if !horizontal_delta_dominates(delta_x, delta_y) {
        return false;
    }
    let offset_before_event = f32::from(scroll.offset().x - delta.x);
    should_capture_nested_horizontal_scroll(
        delta_x,
        offset_before_event,
        f32::from(scroll.max_offset().x),
    )
}
