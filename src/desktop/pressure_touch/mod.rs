use gpui::{MousePressureEvent, PressureStage, Window};

#[cfg(target_os = "macos")]
mod macos;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ForceClickChange {
    None,
    Triggered,
    Released,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ForceClickState {
    triggered: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ForceClickGestureChange<T> {
    None,
    Triggered(T),
    Released(T),
}

#[derive(Clone, Debug)]
pub(crate) struct ForceClickGesture<T> {
    force_click: ForceClickState,
    active_target: Option<T>,
    consumed_target: Option<T>,
}

impl<T> Default for ForceClickGesture<T> {
    fn default() -> Self {
        Self {
            force_click: ForceClickState::default(),
            active_target: None,
            consumed_target: None,
        }
    }
}

impl<T: Clone + PartialEq> ForceClickGesture<T> {
    pub(crate) fn begin(&mut self) {
        self.force_click.cancel();
        self.active_target = None;
        self.consumed_target = None;
    }

    pub(crate) fn update(
        &mut self,
        event: &MousePressureEvent,
        target: T,
    ) -> ForceClickGestureChange<T> {
        self.update_stage(event.stage, target)
    }

    pub(crate) fn cancel(&mut self) -> Option<T> {
        self.force_click.cancel();
        self.active_target.take()
    }

    pub(crate) fn consume_click(&mut self, target: &T) -> bool {
        if self.consumed_target.as_ref() != Some(target) {
            return false;
        }
        self.consumed_target = None;
        true
    }

    fn update_stage(&mut self, stage: PressureStage, target: T) -> ForceClickGestureChange<T> {
        match self.force_click.update_stage(stage) {
            ForceClickChange::Triggered => {
                self.active_target = Some(target.clone());
                self.consumed_target = Some(target.clone());
                ForceClickGestureChange::Triggered(target)
            }
            ForceClickChange::Released => self.active_target.take().map_or(
                ForceClickGestureChange::None,
                ForceClickGestureChange::Released,
            ),
            ForceClickChange::None => ForceClickGestureChange::None,
        }
    }
}

impl ForceClickState {
    pub(crate) fn update(&mut self, event: &MousePressureEvent) -> ForceClickChange {
        self.update_stage(event.stage)
    }

    pub(crate) fn cancel(&mut self) -> ForceClickChange {
        if std::mem::take(&mut self.triggered) {
            ForceClickChange::Released
        } else {
            ForceClickChange::None
        }
    }

    fn update_stage(&mut self, stage: PressureStage) -> ForceClickChange {
        match stage {
            PressureStage::Force if !self.triggered => {
                self.triggered = true;
                ForceClickChange::Triggered
            }
            PressureStage::Zero => self.cancel(),
            PressureStage::Normal | PressureStage::Force => ForceClickChange::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Feedback {
    SelectionChanged,
    Boundary,
}

pub(crate) fn configure(window: &Window) {
    #[cfg(target_os = "macos")]
    macos::configure(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

pub(crate) fn feedback(feedback: Feedback) {
    #[cfg(target_os = "macos")]
    macos::feedback(feedback);
    #[cfg(not(target_os = "macos"))]
    let _ = feedback;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_triggers_once_until_released() {
        let mut state = ForceClickState::default();

        assert_eq!(
            state.update_stage(PressureStage::Force),
            ForceClickChange::Triggered
        );
        assert_eq!(
            state.update_stage(PressureStage::Force),
            ForceClickChange::None
        );
        assert_eq!(
            state.update_stage(PressureStage::Normal),
            ForceClickChange::None
        );
        assert_eq!(
            state.update_stage(PressureStage::Zero),
            ForceClickChange::Released
        );
    }

    #[test]
    fn force_can_trigger_again_after_release() {
        let mut state = ForceClickState::default();

        assert_eq!(
            state.update_stage(PressureStage::Force),
            ForceClickChange::Triggered
        );
        assert_eq!(
            state.update_stage(PressureStage::Zero),
            ForceClickChange::Released
        );
        assert_eq!(
            state.update_stage(PressureStage::Force),
            ForceClickChange::Triggered
        );
    }

    #[test]
    fn cancellation_resets_force_click() {
        let mut state = ForceClickState::default();

        assert_eq!(
            state.update_stage(PressureStage::Force),
            ForceClickChange::Triggered
        );
        assert_eq!(state.cancel(), ForceClickChange::Released);
        assert_eq!(state.cancel(), ForceClickChange::None);
        assert_eq!(
            state.update_stage(PressureStage::Force),
            ForceClickChange::Triggered
        );
    }

    #[test]
    fn release_without_force_is_ignored() {
        let mut state = ForceClickState::default();

        assert_eq!(
            state.update_stage(PressureStage::Normal),
            ForceClickChange::None
        );
        assert_eq!(
            state.update_stage(PressureStage::Zero),
            ForceClickChange::None
        );
    }

    #[test]
    fn targeted_gesture_keeps_click_consumed_after_release() {
        let mut gesture = ForceClickGesture::default();
        gesture.begin();

        assert_eq!(
            gesture.update_stage(PressureStage::Force, "response-a"),
            ForceClickGestureChange::Triggered("response-a")
        );
        assert_eq!(
            gesture.update_stage(PressureStage::Zero, "response-a"),
            ForceClickGestureChange::Released("response-a")
        );
        assert!(gesture.consume_click(&"response-a"));
        assert!(!gesture.consume_click(&"response-a"));
    }

    #[test]
    fn targeted_gesture_cancel_keeps_synthetic_click_consumed() {
        let mut gesture = ForceClickGesture::default();
        gesture.begin();
        gesture.update_stage(PressureStage::Force, "conversation-a");

        assert_eq!(gesture.cancel(), Some("conversation-a"));
        assert!(gesture.consume_click(&"conversation-a"));
    }

    #[test]
    fn next_normal_press_clears_stale_click_consumption() {
        let mut gesture = ForceClickGesture::default();
        gesture.begin();
        gesture.update_stage(PressureStage::Force, "conversation-a");
        gesture.cancel();

        gesture.begin();

        assert!(!gesture.consume_click(&"conversation-a"));
    }
}
