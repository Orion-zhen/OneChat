use super::*;
use crate::desktop::ui::stream::should_capture_nested_scroll;

const USER_MESSAGE_WIDTH_RATIO: f32 = 0.75;
const SENT_IMAGE_MAX_WIDTH: f32 = 520.0;
const SENT_IMAGE_MAX_HEIGHT: f32 = 360.0;

mod assistant;
mod attachments;
mod opening;
mod reasoning;
mod tools;
mod user;

pub(super) use assistant::render_assistant_turn;
use attachments::render_sent_attachment;
pub(super) use opening::render_assistant_opening;
use reasoning::{render_reasoning, render_reasoning_block};
use tools::{render_tool_execution, render_tool_executions, render_tool_placeholder};
pub(super) use user::render_user_turn;

fn user_message_max_width(message_max_width: f32) -> f32 {
    (message_max_width * USER_MESSAGE_WIDTH_RATIO)
        .max(MIN_READABLE_MESSAGE_WIDTH)
        .min(message_max_width)
}

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

fn render_assistant_text_editor(
    id: &str,
    editor: &gpui::Entity<gpui_component::input::TextareaState>,
    label: String,
    aria_label: String,
    typography: MessageTypography,
    cx: &App,
) -> AnyElement {
    animated_editor(
        div()
            .rounded(px(20.0))
            .border_1()
            .border_color(crate::desktop::ui::theme::palette(cx).accent_border)
            .bg(cx.theme().popover)
            .shadow_xs()
            .p_3()
            .child(
                div()
                    .pb_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_color(cx.theme().muted_foreground)
                    .child(render_icon(AppIcon::Pencil, IconTone::Accent, 14.0, cx))
                    .child(
                        div()
                            .text_size(px(typography.metadata_size))
                            .line_height(px(typography.metadata_line_height))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(label),
                    ),
            )
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_move(|_, _, cx| cx.stop_propagation())
                    .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_up_out(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        Textarea::new(editor)
                            .aria_label(aria_label)
                            .appearance(false)
                            .w_full()
                            .text_size(px(typography.body_size))
                            .line_height(px(typography.body_line_height)),
                    ),
            )
            .into_any_element(),
        SharedString::from(format!("assistant-text-editor-{id}")),
        cx,
    )
}

fn render_editor_controls(
    app: &OneChat,
    message: &AssistantResponse,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let response_id = message.id.clone();
    let mouse_down_id = response_id.clone();
    let can_save = app.can_save_assistant_edit(&message.id, cx);
    div()
        .mb_4()
        .flex()
        .items_center()
        .justify_end()
        .gap_2()
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(px(typography.micro_size))
                .line_height(px(typography.micro_line_height))
                .text_color(cx.theme().muted_foreground)
                .child(message_edit_shortcut_hint(app)),
        )
        .child(
            editor_cancel_button(
                SharedString::from(format!("cancel-edit-message-{}", message.id)),
                cx,
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.cancel_message_edit(cx);
                }),
            )
            .on_click(cx.listener(|this, _, _, cx| this.cancel_message_edit(cx))),
        )
        .child(
            editor_save_button(
                SharedString::from(format!("save-edit-message-{}", message.id)),
                "Save response edit",
                !can_save,
                cx,
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.save_assistant_edit(mouse_down_id.clone(), cx);
                }),
            )
            .on_click(
                cx.listener(move |this, _, _, cx| {
                    this.save_assistant_edit(response_id.clone(), cx)
                }),
            ),
        )
        .into_any_element()
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

fn animated_message(message: AnyElement, id: String, highlighted: bool, cx: &App) -> AnyElement {
    div()
        .id(SharedString::from(format!("message-anchor-{id}")))
        .relative()
        .w_full()
        .when(highlighted, |message| {
            message
                .rounded(px(16.0))
                .bg(crate::desktop::ui::theme::palette(cx).accent_soft)
        })
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

#[cfg(test)]
mod tests {
    use super::user_message_max_width;

    #[test]
    fn long_user_messages_expand_in_narrow_conversations() {
        assert_eq!(user_message_max_width(472.0), 472.0);
        assert_eq!(user_message_max_width(480.0), 480.0);
    }

    #[test]
    fn long_user_messages_keep_the_compact_wide_layout() {
        assert_eq!(user_message_max_width(800.0), 600.0);
    }
}
