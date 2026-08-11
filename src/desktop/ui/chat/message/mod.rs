use super::*;
use crate::desktop::ui::stream::should_capture_nested_scroll;
use unicode_segmentation::UnicodeSegmentation;

const USER_MESSAGE_WIDTH_RATIO: f32 = 0.75;
const USER_EDITOR_MIN_WIDTH: f32 = 280.0;
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

fn message_edit_submits(app: &OneChat, action: &Enter) -> bool {
    !action.shift
        && match app.settings().send_message_shortcut {
            SendMessageShortcut::Enter => !action.secondary,
            SendMessageShortcut::SecondaryEnter => action.secondary,
        }
}

fn message_edit_shortcut_hint(app: &OneChat) -> &'static str {
    match app.settings().send_message_shortcut {
        SendMessageShortcut::Enter => "Return to save · Shift–Return for newline",
        SendMessageShortcut::SecondaryEnter if cfg!(target_os = "macos") => {
            "⌘ Return to save · Return for newline"
        }
        SendMessageShortcut::SecondaryEnter => "Ctrl Return to save · Return for newline",
    }
}

fn editor_cancel_button(id: impl Into<ElementId>, cx: &App) -> Button {
    Button::new(id)
        .ghost()
        .rounded(px(17.0))
        .tooltip("Cancel editing (Esc)")
        .size(px(34.0))
        .p_0()
        .child(render_icon(AppIcon::Close, IconTone::Muted, 18.0, cx))
}

fn editor_save_button(
    id: impl Into<ElementId>,
    tooltip: &'static str,
    disabled: bool,
    cx: &App,
) -> Button {
    Button::new(id)
        .primary()
        .rounded(px(17.0))
        .tooltip(tooltip)
        .disabled(disabled)
        .size(px(34.0))
        .p_0()
        .child(render_icon(
            AppIcon::Save,
            if disabled {
                IconTone::Muted
            } else {
                IconTone::OnAccent
            },
            18.0,
            cx,
        ))
}

fn animated_editor(card: AnyElement, id: SharedString, cx: &App) -> AnyElement {
    if cx.reduce_motion() {
        card
    } else {
        div()
            .relative()
            .child(card)
            .with_animation(
                id,
                Animation::new(Duration::from_millis(160)).with_easing(ease_out_quint()),
                |card, delta| {
                    card.opacity(0.72 + delta * 0.28)
                        .top(px(4.0 * (1.0 - delta)))
                },
            )
            .into_any_element()
    }
}

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
