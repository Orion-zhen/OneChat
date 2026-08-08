mod composer;
mod message;
mod system_prompt;

use composer::render_composer;
use message::render_turn;
use system_prompt::render_system_prompt_card;

#[cfg(test)]
use message::{format_message_stats, format_reasoning_duration, user_editor_width};
#[cfg(test)]
use system_prompt::prompt_preview;

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, BoxShadow, Context, ElementId, FontWeight,
    MouseButton, Role, SharedString, div, ease_out_quint, point, prelude::*, px, rgba,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Selectable as _,
    button::{Button, ButtonVariants as _},
    input::{Escape as InputEscape, Input},
};

use super::{
    icons::{AppIcon, IconTone, render_icon},
    markdown,
    motion::translated_y,
    selectable_text::{SelectableText, selection_color},
};
use crate::{
    desktop::app::{COLLAPSED_THINKING_HEIGHT, OneChat, SystemPromptMode},
    domain::{
        AssistantResponse, MAX_MESSAGE_WIDTH_RATIO, MIN_MESSAGE_WIDTH_RATIO, MessageStatus,
        RequestInfo, Turn,
    },
};

const MESSAGE_MIN_WIDTH: f32 = 780.0;
const MESSAGE_LIST_HORIZONTAL_PADDING: f32 = 48.0;

fn response_tab_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id)
        .label(label)
        .h(px(24.0))
        .px_2()
        .rounded(px(8.0))
        .ghost()
        .text_size(px(12.0))
        .font_weight(FontWeight::SEMIBOLD)
}

fn icon_tooltip(icon: AppIcon) -> &'static str {
    match icon {
        AppIcon::ArrowUp => "Send",
        AppIcon::At => "Add response",
        AppIcon::ChevronDown => "Expand",
        AppIcon::ChevronLeft => "Previous",
        AppIcon::ChevronRight => "Next",
        AppIcon::ChevronUp => "Collapse",
        AppIcon::Close => "Cancel",
        AppIcon::ContextSelect => "Use for context",
        AppIcon::Copy => "Copy",
        AppIcon::Fork => "Fork conversation",
        AppIcon::Info => "Inspect request",
        AppIcon::Layers => "Choose model",
        AppIcon::Pencil => "Edit",
        AppIcon::Regenerate => "Regenerate",
        AppIcon::Save => "Save",
        AppIcon::Stop => "Stop generating",
        _ => "Action",
    }
}

fn icon_button(id: impl Into<ElementId>, icon: AppIcon, tone: IconTone, cx: &App) -> Button {
    Button::new(id)
        .ghost()
        .tooltip(icon_tooltip(icon))
        .size(px(28.0))
        .p_0()
        .child(render_icon(icon, tone, 17.0, cx))
}

fn large_icon_button(id: impl Into<ElementId>, icon: AppIcon, tone: IconTone, cx: &App) -> Button {
    Button::new(id)
        .ghost()
        .tooltip(icon_tooltip(icon))
        .size(px(36.0))
        .p_0()
        .child(render_icon(icon, tone, 20.0, cx))
}

fn primary_icon_button(id: impl Into<ElementId>, icon: AppIcon, cx: &App) -> Button {
    Button::new(id)
        .primary()
        .rounded(px(18.0))
        .tooltip(icon_tooltip(icon))
        .size(px(36.0))
        .p_0()
        .child(render_icon(icon, IconTone::OnAccent, 20.0, cx))
}

pub(crate) fn render(
    app: &OneChat,
    available_width: f32,
    scale_factor: f32,
    jump_to_latest_progress: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let conversation = app
        .current_conversation()
        .expect("conversation page requires a current conversation");
    let message_max_width =
        message_max_width(available_width, app.settings().message_width_ratio());
    let has_system_prompt = !conversation.system_prompt.trim().is_empty();
    let editing_system_prompt = app.chat.system_prompt_mode == SystemPromptMode::Editing;
    let text_selection = app.chat.text_selection.clone();
    text_selection.begin_frame();
    let selection_focus = text_selection.focus_handle().clone();
    let selection_mouse_down = text_selection.clone();
    let selection_mouse_move = text_selection.clone();
    let selection_mouse_up = text_selection.clone();
    let selection_mouse_up_out = text_selection.clone();
    let selection_copy = text_selection.clone();
    let mut messages = div()
        .id("message-list")
        .min_h_0()
        .flex_1()
        .overflow_y_scroll()
        .track_scroll(&app.chat.message_scroll)
        .track_focus(&selection_focus)
        .on_scroll_wheel(cx.listener(OneChat::on_message_scroll))
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            selection_mouse_down.mouse_down(event, window, cx)
        })
        .on_mouse_move(move |event, window, _| selection_mouse_move.mouse_move(event, window))
        .on_mouse_up(MouseButton::Left, move |event, window, _| {
            selection_mouse_up.mouse_up(event, window)
        })
        .on_mouse_up_out(MouseButton::Left, move |event, window, _| {
            selection_mouse_up_out.mouse_up(event, window)
        })
        .on_key_down(move |event, window, cx| selection_copy.copy(event, window, cx))
        .px_6()
        .pt_7()
        .pb_6()
        .children(
            (has_system_prompt || editing_system_prompt)
                .then(|| render_system_prompt_card(app, message_max_width, cx)),
        );

    if app.current_turns().is_empty() {
        messages = messages.child(render_empty_conversation(app, cx));
    } else {
        for turn in app.current_turns() {
            messages = messages.child(render_turn(app, turn, message_max_width, scale_factor, cx));
        }
    }

    let jump_to_latest_visible =
        !app.chat.follow_latest && !app.chat.message_scroll_motion.is_active();
    let jump_to_latest_offset = if cx.reduce_motion() {
        0.0
    } else {
        8.0 * (1.0 - jump_to_latest_progress)
    };
    let message_area = div()
        .relative()
        .min_h_0()
        .flex_1()
        .flex()
        .child(messages)
        .children((jump_to_latest_progress > 0.0).then(|| {
            let dark = cx.theme().is_dark();
            let glass = if dark {
                rgba(0x2c2c2ef2)
            } else {
                rgba(0xfffffff2)
            };
            let glass_border = if dark {
                rgba(0xffffff38)
            } else {
                rgba(0x3c3c4326)
            };
            let glass_shadow = vec![BoxShadow {
                color: if dark {
                    rgba(0x0000005c).into()
                } else {
                    rgba(0x1d1d1f24).into()
                },
                offset: point(px(0.0), px(6.0)),
                blur_radius: px(18.0),
                spread_radius: px(-7.0),
                inset: false,
            }];
            translated_y(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom(px(12.0))
                    .flex()
                    .justify_center()
                    .opacity(jump_to_latest_progress)
                    .child(
                        div()
                            .id("jump-to-latest")
                            .rounded(px(20.0))
                            .h(px(40.0))
                            .px_3()
                            .bg(glass)
                            .border_1()
                            .border_color(glass_border)
                            .shadow(glass_shadow)
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .cursor_default()
                            .child(render_icon(
                                AppIcon::ArrowDown,
                                IconTone::Foreground,
                                16.0,
                                cx,
                            ))
                            .child("Jump to Latest")
                            .when(jump_to_latest_visible, |button| {
                                button
                                    .role(Role::Button)
                                    .aria_label("Jump to Latest")
                                    .on_click(cx.listener(|this, _, _, cx| this.jump_to_latest(cx)))
                            }),
                    ),
                px(jump_to_latest_offset),
            )
        }));

    div()
        .size_full()
        .flex()
        .flex_col()
        .child(message_area)
        .child(render_composer(app, message_max_width, cx))
        .into_any_element()
}

fn message_max_width(available_width: f32, ratio: f32) -> f32 {
    let content_width = (available_width - MESSAGE_LIST_HORIZONTAL_PADDING).max(0.0);
    let ratio = ratio.clamp(MIN_MESSAGE_WIDTH_RATIO, MAX_MESSAGE_WIDTH_RATIO);
    (available_width * ratio)
        .max(MESSAGE_MIN_WIDTH)
        .min(content_width)
}

fn render_empty_conversation(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    div()
        .mx_auto()
        .w_full()
        .max_w(px(780.0))
        .min_h(px(300.0))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .max_w(px(520.0))
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .text_center()
                .child(
                    div()
                        .size(px(48.0))
                        .rounded_full()
                        .bg(cx.theme().accent)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(cx.theme().primary)
                        .text_lg()
                        .child(render_icon(AppIcon::Sparkles, IconTone::Accent, 22.0, cx)),
                )
                .child(
                    div()
                        .pt_2()
                        .text_size(px(26.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("What can I help with?"),
                )
                .child(
                    div()
                        .line_height(px(22.0))
                        .text_color(cx.theme().muted_foreground)
                        .child("Ask a question, explore an idea, or make something new."),
                )
                .children(app.current_model().is_none().then(|| {
                    primary_icon_button("empty-choose-model", AppIcon::Layers, cx).on_click(
                        cx.listener(|this, _, window, cx| this.open_model_picker(window, cx)),
                    )
                })),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_editor_expands_for_its_longest_line_before_wrapping() {
        assert_eq!(user_editor_width("给我写一首现代诗", 585.0), 184.0);
        assert_eq!(user_editor_width("short\n给我写一首现代诗", 585.0), 184.0);
    }

    #[test]
    fn user_editor_still_wraps_at_the_user_message_limit() {
        assert_eq!(user_editor_width(&"字".repeat(100), 585.0), 585.0);
    }

    #[test]
    fn message_width_keeps_the_existing_width_as_its_floor() {
        assert_eq!(message_max_width(1_000.0, 0.7), MESSAGE_MIN_WIDTH);
    }

    #[test]
    fn message_width_grows_with_the_available_chat_area() {
        assert_eq!(message_max_width(2_000.0, 0.7), 1_400.0);
    }

    #[test]
    fn message_width_stays_inside_narrow_chat_areas() {
        assert_eq!(message_max_width(800.0, 0.7), 752.0);
    }

    #[test]
    fn prompt_preview_is_short_and_single_line() {
        let preview = prompt_preview(&format!("first\n{}", "x".repeat(200)));
        assert!(!preview.contains('\n'));
        assert!(preview.ends_with('…'));
        assert!(preview.chars().count() <= 161);
    }

    #[test]
    fn reasoning_duration_counts_seconds_to_one_decimal_place() {
        assert_eq!(format_reasoning_duration(0), "0.0s");
        assert_eq!(format_reasoning_duration(999), "0.9s");
        assert_eq!(format_reasoning_duration(65_999), "65.9s");
    }

    #[test]
    fn message_stats_show_output_speed_and_ttft() {
        let mut request = RequestInfo::new("conversation", "turn", "response");
        request.usage.output_tokens = Some(120);
        request.ttft_ms = Some(250);
        request.duration_ms = Some(2_250);

        assert_eq!(
            format_message_stats(&request),
            "120 tokens  ·  60.0 tok/s  ·  TTFT 250 ms"
        );
    }

    #[test]
    fn message_stats_mark_estimated_tokens_and_omit_unavailable_values() {
        let mut request = RequestInfo::new("conversation", "turn", "response");
        request.usage.output_tokens = Some(12);
        request.usage.estimated = true;

        assert_eq!(format_message_stats(&request), "~12 tokens");
    }
}
