use std::time::{Duration, Instant};

use gpui::TouchPhase;

const SWIPE_THRESHOLD: f32 = 72.0;
const AXIS_LOCK_THRESHOLD: f32 = 8.0;
const HORIZONTAL_AXIS_ADVANTAGE: f32 = 1.2;
const MOVED_GESTURE_TIMEOUT: Duration = Duration::from_millis(200);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BranchSwipeTarget {
    User(String),
    Assistant(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BranchSwipeAction {
    Previous,
    Next,
    Boundary,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BranchSwipeAvailability {
    pub previous: bool,
    pub next: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum AxisLock {
    #[default]
    Pending,
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug)]
pub(crate) struct BranchSwipeState<T> {
    active_target: Option<T>,
    accumulated_x: f32,
    accumulated_y: f32,
    axis: AxisLock,
    committed: bool,
    last_event_at: Option<Instant>,
}

impl<T> Default for BranchSwipeState<T> {
    fn default() -> Self {
        Self {
            active_target: None,
            accumulated_x: 0.0,
            accumulated_y: 0.0,
            axis: AxisLock::Pending,
            committed: false,
            last_event_at: None,
        }
    }
}

impl<T: Eq> BranchSwipeState<T> {
    pub(crate) fn update(
        &mut self,
        target: T,
        delta_x: f32,
        delta_y: f32,
        phase: TouchPhase,
        availability: BranchSwipeAvailability,
    ) -> Option<BranchSwipeAction> {
        self.update_at(
            target,
            delta_x,
            delta_y,
            phase,
            availability,
            Instant::now(),
        )
    }

    pub(crate) fn reset(&mut self) {
        self.active_target = None;
        self.accumulated_x = 0.0;
        self.accumulated_y = 0.0;
        self.axis = AxisLock::Pending;
        self.committed = false;
        self.last_event_at = None;
    }

    fn update_at(
        &mut self,
        target: T,
        delta_x: f32,
        delta_y: f32,
        phase: TouchPhase,
        availability: BranchSwipeAvailability,
        now: Instant,
    ) -> Option<BranchSwipeAction> {
        match phase {
            TouchPhase::Ended | TouchPhase::Cancelled => {
                self.reset();
                return None;
            }
            TouchPhase::Started => self.begin(target, now),
            TouchPhase::Moved => {
                let timed_out = self.last_event_at.is_some_and(|last| {
                    now.saturating_duration_since(last) > MOVED_GESTURE_TIMEOUT
                });
                if self.active_target.as_ref() != Some(&target) || timed_out {
                    self.begin(target, now);
                } else {
                    self.last_event_at = Some(now);
                }
            }
        }

        if self.committed {
            return None;
        }

        self.accumulated_x += delta_x;
        self.accumulated_y += delta_y;
        self.lock_axis();
        if self.axis != AxisLock::Horizontal || self.accumulated_x.abs() < SWIPE_THRESHOLD {
            return None;
        }

        self.committed = true;
        Some(if self.accumulated_x < 0.0 {
            if availability.next {
                BranchSwipeAction::Next
            } else {
                BranchSwipeAction::Boundary
            }
        } else if availability.previous {
            BranchSwipeAction::Previous
        } else {
            BranchSwipeAction::Boundary
        })
    }

    fn begin(&mut self, target: T, now: Instant) {
        self.active_target = Some(target);
        self.accumulated_x = 0.0;
        self.accumulated_y = 0.0;
        self.axis = AxisLock::Pending;
        self.committed = false;
        self.last_event_at = Some(now);
    }

    fn lock_axis(&mut self) {
        if self.axis != AxisLock::Pending {
            return;
        }
        let horizontal = self.accumulated_x.abs();
        let vertical = self.accumulated_y.abs();
        if horizontal.max(vertical) < AXIS_LOCK_THRESHOLD {
            return;
        }
        if horizontal_delta_dominates(horizontal, vertical) {
            self.axis = AxisLock::Horizontal;
        } else if vertical >= horizontal * HORIZONTAL_AXIS_ADVANTAGE {
            self.axis = AxisLock::Vertical;
        }
    }
}

pub(crate) fn horizontal_delta_dominates(delta_x: f32, delta_y: f32) -> bool {
    delta_x.abs() >= delta_y.abs() * HORIZONTAL_AXIS_ADVANTAGE
}

pub(crate) fn should_capture_nested_horizontal_scroll(
    delta_x: f32,
    offset_x: f32,
    max_offset_x: f32,
) -> bool {
    if delta_x > 0.0 {
        offset_x < 0.0
    } else if delta_x < 0.0 {
        max_offset_x + offset_x > 0.0
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
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
}
