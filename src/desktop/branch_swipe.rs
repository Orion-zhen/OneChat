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

    pub(crate) fn captures_parent_scroll(&self) -> bool {
        self.active_target.is_some() && self.axis != AxisLock::Vertical
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
mod tests;
