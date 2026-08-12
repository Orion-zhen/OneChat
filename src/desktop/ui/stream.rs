use std::{cell::RefCell, collections::HashMap};

use gpui::{ScrollDelta, ScrollHandle, ScrollWheelEvent};

use crate::desktop::branch_swipe::{
    horizontal_delta_dominates, should_capture_nested_horizontal_scroll,
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
