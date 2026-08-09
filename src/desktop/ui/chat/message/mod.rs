use super::*;
use crate::desktop::ui::stream::should_capture_nested_scroll;
use unicode_segmentation::UnicodeSegmentation;

const USER_MESSAGE_WIDTH_RATIO: f32 = 0.75;
const USER_EDITOR_MIN_WIDTH: f32 = 160.0;
const USER_EDITOR_HORIZONTAL_CHROME: f32 = 64.0;
const USER_EDITOR_MEASUREMENT_FONT_SIZE: f32 = 15.0;
const SENT_IMAGE_MAX_WIDTH: f32 = 520.0;
const SENT_IMAGE_MAX_HEIGHT: f32 = 360.0;

mod assistant;
mod attachments;
mod reasoning;
mod tools;
mod user;

pub(super) use assistant::render_assistant_turn;
use attachments::{render_sent_attachment, user_editor_width};
use reasoning::render_reasoning;
use tools::render_tool_executions;
pub(super) use user::render_user_turn;

fn animated_message(message: AnyElement, id: String) -> AnyElement {
    div()
        .id(SharedString::from(format!("message-anchor-{id}")))
        .relative()
        .w_full()
        .child(message)
        .with_animation(
            SharedString::from(format!("message-appear-{id}")),
            Animation::new(Duration::from_millis(240)).with_easing(ease_out_quint()),
            |message, delta| {
                message
                    .opacity(0.72 + delta * 0.28)
                    .top(px(6.0 * (1.0 - delta)))
            },
        )
        .into_any_element()
}
