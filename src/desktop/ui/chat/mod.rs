mod composer;
mod message;
mod system_prompt;

use composer::render_composer;
use message::render_message;
use system_prompt::render_system_prompt_card;

#[cfg(test)]
use message::{format_message_stats, format_reasoning_duration};
#[cfg(test)]
use system_prompt::prompt_preview;

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, Context, FontWeight, SharedString, div,
    ease_out_quint, prelude::*, px, rgba,
};

use super::{
    components::{
        IconTone, UiIcon, button, compact_button, destructive_icon_button, primary_button,
        primary_icon_button, svg_icon_button,
    },
    inspector::InspectorTab,
    markdown,
    theme::Colors,
};
use crate::{
    desktop::app::{COLLAPSED_THINKING_HEIGHT, OneChat, SystemPromptMode},
    domain::{Message, MessageRole, MessageStatus, RequestInfo, SystemPromptSource},
};

pub(crate) fn render(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let conversation = app
        .current_conversation()
        .expect("conversation page requires a current conversation");
    let has_system_prompt = !conversation.system_prompt.content.trim().is_empty();
    let editing_system_prompt = app.chat.system_prompt_mode == SystemPromptMode::Editing;
    let mut messages = div()
        .id("message-list")
        .min_h_0()
        .flex_1()
        .overflow_y_scroll()
        .track_scroll(&app.chat.message_scroll)
        .on_scroll_wheel(cx.listener(OneChat::on_message_scroll))
        .px_6()
        .pt_7()
        .pb_6()
        .children(
            (has_system_prompt || editing_system_prompt)
                .then(|| render_system_prompt_card(app, colors, cx)),
        );

    if app.current_messages().is_empty() {
        messages = messages.child(render_empty_conversation(app, colors, cx));
    } else {
        for message in app.current_messages() {
            messages = messages.child(render_message(app, message, colors, scale_factor, cx));
        }
    }

    let message_area = div()
        .relative()
        .min_h_0()
        .flex_1()
        .flex()
        .child(messages)
        .children((!app.chat.follow_latest).then(|| {
            let glass = if colors.dark {
                rgba(0x2c2c2ed9)
            } else {
                rgba(0xffffffd9)
            };
            let glass_hover = if colors.dark {
                rgba(0x3a3a3cef)
            } else {
                rgba(0xfffffff2)
            };
            let glass_border = if colors.dark {
                rgba(0xffffff2b)
            } else {
                rgba(0x3c3c4324)
            };
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(12.0))
                .flex()
                .justify_center()
                .child(
                    div()
                        .id("jump-to-latest")
                        .relative()
                        .h(px(36.0))
                        .px_4()
                        .rounded_full()
                        .border_1()
                        .border_color(glass_border)
                        .bg(glass)
                        .shadow_md()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .hover(move |style| style.bg(glass_hover))
                        .active(move |style| style.bg(colors.accent_soft).text_color(colors.accent))
                        .child(div().text_base().line_height(px(16.0)).child("↓"))
                        .child("Jump to Latest")
                        .on_click(cx.listener(|this, _, _, cx| this.jump_to_latest(cx)))
                        .with_animation(
                            "jump-to-latest-appear",
                            Animation::new(Duration::from_millis(180))
                                .with_easing(ease_out_quint()),
                            |button, delta| {
                                button
                                    .opacity(0.72 + delta * 0.28)
                                    .top(px(6.0 * (1.0 - delta)))
                            },
                        ),
                )
        }));

    div()
        .size_full()
        .flex()
        .flex_col()
        .child(message_area)
        .child(render_composer(
            app,
            has_system_prompt,
            editing_system_prompt,
            colors,
            cx,
        ))
        .into_any_element()
}

fn render_empty_conversation(
    app: &OneChat,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
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
                        .bg(colors.accent_soft)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(colors.accent)
                        .text_lg()
                        .child("✦"),
                )
                .child(
                    div()
                        .pt_2()
                        .text_size(px(22.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Start a Conversation"),
                )
                .child(
                    div()
                        .line_height(px(22.0))
                        .text_color(colors.muted)
                        .child("Ask a question, compare ideas, or work through something new."),
                )
                .children(app.current_model().is_none().then(|| {
                    primary_button("empty-choose-model", "Choose Model", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.open_model_picker(cx)))
                })),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut request = RequestInfo::new("conversation", "message");
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
        let mut request = RequestInfo::new("conversation", "message");
        request.usage.output_tokens = Some(12);
        request.usage.estimated = true;

        assert_eq!(format_message_stats(&request), "~12 tokens");
    }
}
