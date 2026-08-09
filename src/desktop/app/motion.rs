use std::time::{Duration, Instant};

use gpui::Window;

const DRAWER_RESPONSE_SECONDS: f32 = 0.34;
const DRAWER_DAMPING_RATIO: f32 = 1.0;
const SIDEBAR_WIDTH_RESPONSE_SECONDS: f32 = 0.4;
const SIDEBAR_WIDTH_DAMPING_RATIO: f32 = 1.0;

pub(crate) struct DrawerMotion {
    value: f32,
    velocity: f32,
    target: f32,
    last_frame: Option<Instant>,
}

const VISIBILITY_MOTION_DURATION: Duration = Duration::from_millis(180);

pub(crate) struct VisibilityMotion {
    value: f32,
    from: f32,
    target: f32,
    started_at: Option<Instant>,
}

impl VisibilityMotion {
    pub(crate) fn new(visible: bool) -> Self {
        let value = f32::from(visible);
        Self {
            value,
            from: value,
            target: value,
            started_at: None,
        }
    }

    pub(crate) fn set_visible(&mut self, visible: bool) {
        self.set_visible_at(visible, Instant::now());
    }

    fn set_visible_at(&mut self, visible: bool, now: Instant) {
        self.advance(now);
        let target = f32::from(visible);
        if (target - self.target).abs() < f32::EPSILON {
            return;
        }

        self.from = self.value;
        self.target = target;
        self.started_at = Some(now);
    }

    pub(crate) fn progress(&mut self, window: &mut Window, reduce_motion: bool) -> f32 {
        if reduce_motion {
            self.snap();
            return self.value;
        }
        self.advance(Instant::now());
        if self.started_at.is_some() {
            window.request_animation_frame();
        }
        self.value
    }

    fn snap(&mut self) {
        self.value = self.target;
        self.from = self.target;
        self.started_at = None;
    }

    fn advance(&mut self, now: Instant) {
        let Some(started_at) = self.started_at else {
            return;
        };
        let delta = (now - started_at).as_secs_f32() / VISIBILITY_MOTION_DURATION.as_secs_f32();
        if delta >= 1.0 {
            self.snap();
            return;
        }

        let eased = gpui::ease_out_quint()(delta);
        self.value = self.from + (self.target - self.from) * eased;
    }
}

impl DrawerMotion {
    pub(crate) fn new(open: bool) -> Self {
        let value = f32::from(open);
        Self {
            value,
            velocity: 0.0,
            target: value,
            last_frame: None,
        }
    }

    pub(crate) fn set_open(&mut self, open: bool, animated: bool) {
        let now = Instant::now();
        self.advance(now);
        let target = f32::from(open);
        if (target - self.target).abs() < f32::EPSILON {
            return;
        }
        if !animated {
            self.snap(open);
            return;
        }

        self.target = target;
        self.last_frame = Some(now);
    }

    pub(crate) fn snap(&mut self, open: bool) {
        let value = f32::from(open);
        self.value = value;
        self.velocity = 0.0;
        self.target = value;
        self.last_frame = None;
    }

    pub(crate) fn progress(&mut self, window: &mut Window, reduce_motion: bool) -> f32 {
        if reduce_motion {
            self.snap(self.target > 0.5);
            return self.value;
        }
        self.advance(Instant::now());
        if self.last_frame.is_some() {
            window.request_animation_frame();
        }
        self.value
    }

    fn advance(&mut self, now: Instant) {
        let Some(last_frame) = self.last_frame else {
            return;
        };

        let elapsed = (now - last_frame).as_secs_f32().min(0.064);
        self.last_frame = Some(now);
        self.step(elapsed);
    }

    fn step(&mut self, elapsed: f32) {
        let steps = (elapsed / (1.0 / 120.0)).ceil().max(1.0) as usize;
        let delta = elapsed / steps as f32;
        let omega = std::f32::consts::TAU / DRAWER_RESPONSE_SECONDS;
        for _ in 0..steps {
            let acceleration = omega * omega * (self.target - self.value)
                - 2.0 * DRAWER_DAMPING_RATIO * omega * self.velocity;
            self.velocity += acceleration * delta;
            self.value += self.velocity * delta;
        }

        self.value = self.value.clamp(0.0, 1.0);
        if (self.target - self.value).abs() < 0.001 && self.velocity.abs() < 0.001 {
            self.value = self.target;
            self.velocity = 0.0;
            self.last_frame = None;
        }
    }
}

pub(crate) struct SidebarWidthMotion {
    value: f32,
    velocity: f32,
    target: f32,
    last_frame: Option<Instant>,
}

impl SidebarWidthMotion {
    pub(crate) fn new(width: f32) -> Self {
        Self {
            value: width,
            velocity: 0.0,
            target: width,
            last_frame: None,
        }
    }

    pub(crate) fn set_target(&mut self, width: f32, animated: bool) {
        let now = Instant::now();
        self.advance(now);
        if (width - self.target).abs() < f32::EPSILON {
            return;
        }
        if !animated {
            self.snap(width);
            return;
        }

        self.target = width;
        self.last_frame = Some(now);
    }

    pub(crate) fn snap(&mut self, width: f32) {
        self.value = width;
        self.velocity = 0.0;
        self.target = width;
        self.last_frame = None;
    }

    pub(crate) fn progress(&mut self, window: &mut Window, reduce_motion: bool) -> f32 {
        if reduce_motion {
            self.snap(self.target);
            return self.value;
        }
        self.advance(Instant::now());
        if self.last_frame.is_some() {
            window.request_animation_frame();
        }
        self.value
    }

    fn advance(&mut self, now: Instant) {
        let Some(last_frame) = self.last_frame else {
            return;
        };

        let elapsed = (now - last_frame).as_secs_f32().min(0.064);
        self.last_frame = Some(now);
        let steps = (elapsed / (1.0 / 120.0)).ceil().max(1.0) as usize;
        let delta = elapsed / steps as f32;
        let omega = std::f32::consts::TAU / SIDEBAR_WIDTH_RESPONSE_SECONDS;
        for _ in 0..steps {
            let acceleration = omega * omega * (self.target - self.value)
                - 2.0 * SIDEBAR_WIDTH_DAMPING_RATIO * omega * self.velocity;
            self.velocity += acceleration * delta;
            self.value += self.velocity * delta;
        }

        if (self.target - self.value).abs() < 0.05 && self.velocity.abs() < 0.05 {
            self.snap(self.target);
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ThinkingMotion {
    pub(crate) from_height: f32,
    pub(crate) full_height: f32,
}

const MESSAGE_SCROLL_DURATION: Duration = Duration::from_millis(250);

pub(crate) struct MessageScrollMotion {
    from: f32,
    target: f32,
    settle_at_bottom: bool,
    started_at: Option<Instant>,
}

impl MessageScrollMotion {
    pub(crate) fn new() -> Self {
        Self {
            from: 0.0,
            target: 0.0,
            settle_at_bottom: false,
            started_at: None,
        }
    }

    pub(crate) fn start(&mut self, from: f32, target: f32, settle_at_bottom: bool) {
        self.from = from;
        self.target = target;
        self.settle_at_bottom = settle_at_bottom;
        self.started_at = Some(Instant::now());
    }

    pub(crate) fn cancel(&mut self) {
        self.started_at = None;
    }

    pub(crate) fn is_active(&self) -> bool {
        self.started_at.is_some()
    }

    pub(crate) fn offset(&mut self, window: &mut Window) -> Option<(f32, bool, bool)> {
        let started_at = self.started_at?;
        let delta = started_at.elapsed().as_secs_f32() / MESSAGE_SCROLL_DURATION.as_secs_f32();
        if delta >= 1.0 {
            self.started_at = None;
            return Some((self.target, true, self.settle_at_bottom));
        }

        window.request_animation_frame();
        let progress = strong_ease_in_out(delta);
        Some((
            self.from + (self.target - self.from) * progress,
            false,
            self.settle_at_bottom,
        ))
    }
}

fn strong_ease_in_out(delta: f32) -> f32 {
    let target_x = delta.clamp(0.0, 1.0);
    if target_x == 0.0 || target_x == 1.0 {
        return target_x;
    }

    let mut lower = 0.0;
    let mut upper = 1.0;
    for _ in 0..16 {
        let time = (lower + upper) / 2.0;
        if cubic_bezier_coordinate(time, 0.77, 0.175) < target_x {
            lower = time;
        } else {
            upper = time;
        }
    }
    cubic_bezier_coordinate((lower + upper) / 2.0, 0.0, 1.0)
}

fn cubic_bezier_coordinate(time: f32, control_1: f32, control_2: f32) -> f32 {
    let inverse = 1.0 - time;
    3.0 * inverse * inverse * time * control_1
        + 3.0 * inverse * time * time * control_2
        + time * time * time
}
