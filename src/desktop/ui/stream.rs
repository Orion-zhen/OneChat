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
