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
}
