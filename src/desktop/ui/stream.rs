pub fn follow_after_scroll(current: bool, delta_y: f32, distance_from_bottom: f32) -> bool {
    if delta_y > 0.0 {
        false
    } else if delta_y < 0.0 {
        current || distance_from_bottom + delta_y <= 48.0
    } else {
        current
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
}
