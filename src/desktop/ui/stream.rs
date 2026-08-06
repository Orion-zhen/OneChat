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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_follow_stops_on_upward_scroll_and_resumes_at_the_bottom() {
        assert!(!follow_after_scroll(true, 20.0, 0.0));
        assert!(!follow_after_scroll(false, -20.0, 200.0));
        assert!(follow_after_scroll(false, -20.0, 60.0));
        assert!(follow_after_scroll(true, -20.0, 0.0));
    }

    #[test]
    fn nested_scroll_only_captures_events_while_it_can_move() {
        assert!(should_capture_nested_scroll(20.0, -50.0, 100.0));
        assert!(!should_capture_nested_scroll(20.0, 0.0, 100.0));
        assert!(should_capture_nested_scroll(-20.0, -50.0, 100.0));
        assert!(!should_capture_nested_scroll(-20.0, -100.0, 100.0));
        assert!(!should_capture_nested_scroll(0.0, -50.0, 100.0));
    }
}
