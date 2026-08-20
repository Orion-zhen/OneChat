use super::*;

const BOTH_DIRECTIONS: BranchSwipeAvailability = BranchSwipeAvailability {
    previous: true,
    next: true,
};

fn moved(
    state: &mut BranchSwipeState<&'static str>,
    target: &'static str,
    x: f32,
    y: f32,
    availability: BranchSwipeAvailability,
    now: Instant,
) -> Option<BranchSwipeAction> {
    state.update_at(target, x, y, TouchPhase::Moved, availability, now)
}

#[test]
fn left_swipe_submits_next_and_right_swipe_submits_previous() {
    let now = Instant::now();
    let mut state = BranchSwipeState::default();
    assert_eq!(
        moved(&mut state, "a", -72.0, 0.0, BOTH_DIRECTIONS, now),
        Some(BranchSwipeAction::Next)
    );

    state.reset();
    assert_eq!(
        moved(&mut state, "a", 72.0, 0.0, BOTH_DIRECTIONS, now),
        Some(BranchSwipeAction::Previous)
    );
}

#[test]
fn movement_below_threshold_does_not_submit() {
    let mut state = BranchSwipeState::default();
    assert_eq!(
        moved(&mut state, "a", -71.9, 0.0, BOTH_DIRECTIONS, Instant::now()),
        None
    );
}

#[test]
fn pending_and_horizontal_gestures_capture_parent_scroll() {
    let now = Instant::now();
    let mut state = BranchSwipeState::default();
    assert_eq!(
        moved(&mut state, "a", -2.0, 1.0, BOTH_DIRECTIONS, now),
        None
    );
    assert!(state.captures_parent_scroll());
    assert_eq!(
        moved(
            &mut state,
            "a",
            -10.0,
            1.0,
            BOTH_DIRECTIONS,
            now + Duration::from_millis(10)
        ),
        None
    );
    assert!(state.captures_parent_scroll());
}

#[test]
fn vertical_gesture_releases_parent_scroll() {
    let mut state = BranchSwipeState::default();
    assert_eq!(
        moved(&mut state, "a", 1.0, 8.0, BOTH_DIRECTIONS, Instant::now()),
        None
    );
    assert!(!state.captures_parent_scroll());
}

#[test]
fn vertical_gesture_never_becomes_a_swipe() {
    let now = Instant::now();
    let mut state = BranchSwipeState::default();
    assert_eq!(
        moved(&mut state, "a", -4.0, 12.0, BOTH_DIRECTIONS, now),
        None
    );
    assert_eq!(
        moved(
            &mut state,
            "a",
            -100.0,
            0.0,
            BOTH_DIRECTIONS,
            now + Duration::from_millis(10)
        ),
        None
    );
}

#[test]
fn one_gesture_submits_only_once() {
    let now = Instant::now();
    let mut state = BranchSwipeState::default();
    assert_eq!(
        moved(&mut state, "a", -72.0, 0.0, BOTH_DIRECTIONS, now),
        Some(BranchSwipeAction::Next)
    );
    assert_eq!(
        moved(
            &mut state,
            "a",
            -72.0,
            0.0,
            BOTH_DIRECTIONS,
            now + Duration::from_millis(10)
        ),
        None
    );
}

#[test]
fn ended_and_cancelled_reset_the_gesture() {
    for phase in [TouchPhase::Ended, TouchPhase::Cancelled] {
        let now = Instant::now();
        let mut state = BranchSwipeState::default();
        assert_eq!(
            moved(&mut state, "a", -72.0, 0.0, BOTH_DIRECTIONS, now),
            Some(BranchSwipeAction::Next)
        );
        assert_eq!(
            state.update_at("a", 0.0, 0.0, phase, BOTH_DIRECTIONS, now),
            None
        );
        assert_eq!(
            moved(
                &mut state,
                "a",
                -72.0,
                0.0,
                BOTH_DIRECTIONS,
                now + Duration::from_millis(10)
            ),
            Some(BranchSwipeAction::Next)
        );
    }
}

#[test]
fn changing_target_starts_a_new_gesture() {
    let now = Instant::now();
    let mut state = BranchSwipeState::default();
    assert_eq!(
        moved(&mut state, "a", -40.0, 0.0, BOTH_DIRECTIONS, now),
        None
    );
    assert_eq!(
        moved(
            &mut state,
            "b",
            -40.0,
            0.0,
            BOTH_DIRECTIONS,
            now + Duration::from_millis(10)
        ),
        None
    );
}

#[test]
fn separated_moved_only_events_start_a_new_gesture() {
    let now = Instant::now();
    let mut state = BranchSwipeState::default();
    assert_eq!(
        moved(&mut state, "a", -72.0, 0.0, BOTH_DIRECTIONS, now),
        Some(BranchSwipeAction::Next)
    );
    assert_eq!(
        moved(
            &mut state,
            "a",
            -72.0,
            0.0,
            BOTH_DIRECTIONS,
            now + MOVED_GESTURE_TIMEOUT + Duration::from_millis(1)
        ),
        Some(BranchSwipeAction::Next)
    );
}

#[test]
fn unavailable_direction_reports_one_boundary() {
    let now = Instant::now();
    let mut state = BranchSwipeState::default();
    let only_previous = BranchSwipeAvailability {
        previous: true,
        next: false,
    };
    assert_eq!(
        moved(&mut state, "a", -72.0, 0.0, only_previous, now),
        Some(BranchSwipeAction::Boundary)
    );
    assert_eq!(
        moved(
            &mut state,
            "a",
            -72.0,
            0.0,
            only_previous,
            now + Duration::from_millis(10)
        ),
        None
    );
}

#[test]
fn nested_scroller_captures_in_both_directions_from_the_middle() {
    assert!(should_capture_nested_horizontal_scroll(10.0, -50.0, 100.0));
    assert!(should_capture_nested_horizontal_scroll(-10.0, -50.0, 100.0));
}

#[test]
fn nested_scroller_at_start_only_captures_toward_its_end() {
    assert!(!should_capture_nested_horizontal_scroll(10.0, 0.0, 100.0));
    assert!(should_capture_nested_horizontal_scroll(-10.0, 0.0, 100.0));
}

#[test]
fn nested_scroller_at_end_only_captures_toward_its_start() {
    assert!(should_capture_nested_horizontal_scroll(10.0, -100.0, 100.0));
    assert!(!should_capture_nested_horizontal_scroll(
        -10.0, -100.0, 100.0
    ));
}
