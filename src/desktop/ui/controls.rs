use gpui::{Context, Entity, Window};
use gpui_component::slider::SliderState;

use crate::desktop::app::OneChat;

pub(super) fn sync_slider(
    slider: &Entity<SliderState>,
    value: f32,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) {
    if (slider.read(cx).value().start() - value).abs() > f32::EPSILON {
        slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
    }
}
