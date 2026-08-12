use gpui::Window;
use objc2::AnyThread as _;
use objc2_app_kit::{
    NSHapticFeedbackManager, NSHapticFeedbackPattern, NSHapticFeedbackPerformanceTime,
    NSHapticFeedbackPerformer as _, NSPressureBehavior, NSPressureConfiguration, NSView,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use super::Feedback;

pub(super) fn configure(window: &Window) {
    let view = native_view(window);
    let configuration = NSPressureConfiguration::initWithPressureBehavior(
        NSPressureConfiguration::alloc(),
        NSPressureBehavior::PrimaryDeepClick,
    );
    view.setPressureConfiguration(Some(&configuration));
    debug_assert_eq!(
        view.pressureConfiguration()
            .map(|configuration| configuration.pressureBehavior()),
        Some(NSPressureBehavior::PrimaryDeepClick)
    );
}

pub(super) fn feedback(feedback: Feedback) {
    let pattern = match feedback {
        Feedback::SelectionChanged => NSHapticFeedbackPattern::Alignment,
        Feedback::Boundary => NSHapticFeedbackPattern::LevelChange,
    };
    NSHapticFeedbackManager::defaultPerformer()
        .performFeedbackPattern_performanceTime(pattern, NSHapticFeedbackPerformanceTime::Now);
}

fn native_view(window: &Window) -> &NSView {
    let window_handle =
        HasWindowHandle::window_handle(window).expect("macOS window must expose an AppKit handle");
    let RawWindowHandle::AppKit(handle) = window_handle.as_raw() else {
        unreachable!("macOS window must use an AppKit handle");
    };
    // SAFETY: GPUI documents the AppKit raw handle as its live backing NSView.
    unsafe { handle.ns_view.cast::<NSView>().as_ref() }
}
