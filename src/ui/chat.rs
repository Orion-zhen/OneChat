use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, Context, FontWeight, SharedString, div,
    ease_out_quint, prelude::*, px, rgba,
};

use crate::{
    app::{OneChat, SystemPromptMode},
    model::{Message, MessageRole, MessageStatus, RequestInfo, SystemPromptSource},
    ui::{
        inspector::InspectorTab,
        markdown,
        shell::{
            Colors, IconTone, UiIcon, button, compact_button, destructive_icon_button,
            primary_button, primary_icon_button, svg_icon_button,
        },
    },
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
    let editing_system_prompt = app.system_prompt_mode == SystemPromptMode::Editing;
    let mut messages = div()
        .id("message-list")
        .min_h_0()
        .flex_1()
        .overflow_y_scroll()
        .track_scroll(&app.message_scroll)
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
        .children((!app.follow_latest).then(|| {
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

fn render_message(
    app: &OneChat,
    message: &Message,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    match message.role {
        MessageRole::User => render_user_message(message, colors),
        MessageRole::Assistant => render_assistant_message(app, message, colors, scale_factor, cx),
    }
}

fn render_user_message(message: &Message, colors: Colors) -> AnyElement {
    div()
        .mx_auto()
        .mb_7()
        .w_full()
        .max_w(px(780.0))
        .flex()
        .justify_end()
        .child(
            div()
                .max_w(px(590.0))
                .rounded_xl()
                .bg(colors.accent)
                .px_4()
                .py_3()
                .text_color(colors.on_accent)
                .whitespace_normal()
                .line_height(px(23.0))
                .child(message.content.clone()),
        )
        .into_any_element()
}

fn render_assistant_message(
    app: &OneChat,
    message: &Message,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let request = app.request_for_message(message);
    let waiting = message.content.is_empty()
        && matches!(
            message.status,
            MessageStatus::Pending | MessageStatus::Streaming
        );
    let editor = app.assistant_message_editor(message);
    let editing = editor.is_some();
    let editing_any = app.active_message_editor().is_some();
    let content = if let Some(editor) = editor {
        let save_id = message.id.clone();
        div()
            .rounded_xl()
            .border_1()
            .border_color(colors.border)
            .bg(colors.panel)
            .p_3()
            .child(editor)
            .child(
                div()
                    .pt_3()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .child(
                        button(
                            SharedString::from(format!("cancel-edit-message-{}", message.id)),
                            "Cancel",
                            colors,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_assistant_edit(cx))),
                    )
                    .child(
                        primary_button(
                            SharedString::from(format!("save-edit-message-{}", message.id)),
                            "Save",
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.save_assistant_edit(save_id.clone(), cx)
                        })),
                    ),
            )
            .into_any_element()
    } else if waiting {
        div()
            .flex()
            .items_center()
            .gap_2()
            .text_color(colors.muted)
            .child(div().size(px(7.0)).rounded_full().bg(colors.accent))
            .child(if message.thinking.is_empty() {
                "Contacting provider…"
            } else {
                "Thinking…"
            })
            .into_any_element()
    } else if let Some(document) = app.markdown_for(message) {
        markdown::render(document, colors, scale_factor)
    } else {
        markdown::render_plain(&message.content, colors)
    };

    let latest = app.is_latest_assistant(&message.id);
    let generating = app.is_current_generating();
    let copy_id = message.id.clone();
    let edit_id = message.id.clone();
    let regenerate_id = message.id.clone();
    let info_id = message.id.clone();
    let thinking_expanded = app.thinking_expanded(&message.id);
    let thinking_id = message.id.clone();
    let thinking_preview = thinking_preview(&message.thinking);
    let mut actions = div().flex().items_center().gap_1();
    if !message.content.is_empty() {
        actions = actions.child(
            svg_icon_button(
                SharedString::from(format!("copy-message-{}", message.id)),
                UiIcon::Copy,
                IconTone::Muted,
                colors,
                scale_factor,
            )
            .on_click(cx.listener(move |this, _, _, cx| this.copy_assistant(copy_id.clone(), cx))),
        );
    }
    if !generating && (!editing_any || editing) {
        actions = actions.child(
            svg_icon_button(
                SharedString::from(format!("edit-message-{}", message.id)),
                UiIcon::Pencil,
                if editing {
                    IconTone::Accent
                } else {
                    IconTone::Muted
                },
                colors,
                scale_factor,
            )
            .on_click(
                cx.listener(move |this, _, _, cx| this.begin_edit_assistant(edit_id.clone(), cx)),
            ),
        );
    }
    if latest
        && !generating
        && !editing
        && !matches!(
            message.status,
            MessageStatus::Failed | MessageStatus::Interrupted
        )
    {
        actions = actions.child(
            svg_icon_button(
                SharedString::from(format!("regenerate-message-{}", message.id)),
                UiIcon::Regenerate,
                IconTone::Muted,
                colors,
                scale_factor,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.regenerate_assistant(regenerate_id.clone(), cx)
            })),
        );
    }
    if request.is_some() {
        actions =
            actions.child(
                svg_icon_button(
                    SharedString::from(format!("info-message-{}", message.id)),
                    UiIcon::Info,
                    IconTone::Muted,
                    colors,
                    scale_factor,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.inspect_message_request(info_id.clone(), cx)
                })),
            );
    }

    let stats = request.map(format_message_stats).unwrap_or_default();
    let show_status = !matches!(message.status, MessageStatus::Completed);
    div()
        .id(SharedString::from(format!(
            "assistant-message-{}",
            message.id
        )))
        .mx_auto()
        .mb_8()
        .w_full()
        .max_w(px(780.0))
        .child(
            div()
                .mb_3()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .size(px(24.0))
                        .rounded_lg()
                        .bg(colors.accent_soft)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(11.0))
                        .text_color(colors.accent)
                        .child("✦"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors.muted)
                        .child("OneChat"),
                )
                .children(show_status.then(|| status_badge(message.status, colors))),
        )
        .children((!message.thinking.is_empty()).then(|| {
            div()
                .mb_4()
                .rounded_xl()
                .bg(colors.raised)
                .p_4()
                .text_sm()
                .line_height(px(22.0))
                .text_color(colors.muted)
                .child(
                    div()
                        .pb_2()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("REASONING"),
                        )
                        .child(
                            compact_button(
                                SharedString::from(format!("thinking-{}", message.id)),
                                if thinking_expanded { "Hide" } else { "Show" },
                                colors,
                            )
                            .text_color(colors.accent)
                            .on_click(cx.listener(
                                move |this, _, _, cx| this.toggle_thinking(thinking_id.clone(), cx),
                            )),
                        ),
                )
                .child(if thinking_expanded {
                    message.thinking.clone()
                } else {
                    thinking_preview
                })
        }))
        .child(content)
        .children(render_error_card(
            app, message, request, latest, generating, colors, cx,
        ))
        .child(
            div()
                .mt_3()
                .min_h(px(24.0))
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(actions)
                .children((!stats.is_empty()).then(|| {
                    div()
                        .min_w_0()
                        .flex_1()
                        .text_right()
                        .text_size(px(11.0))
                        .text_color(colors.muted)
                        .child(stats)
                })),
        )
        .into_any_element()
}

fn format_message_stats(request: &RequestInfo) -> String {
    let mut stats = Vec::new();
    if let Some(tokens) = request.usage.output_tokens {
        stats.push(format!(
            "{}{tokens} tokens",
            if request.usage.estimated { "~" } else { "" }
        ));
        if let (Some(duration_ms), Some(ttft_ms)) = (request.duration_ms, request.ttft_ms) {
            let generation_ms = duration_ms.saturating_sub(ttft_ms);
            if generation_ms > 0 {
                stats.push(format!(
                    "{:.1} tok/s",
                    tokens as f64 * 1000.0 / generation_ms as f64
                ));
            }
        }
    }
    if let Some(ttft_ms) = request.ttft_ms {
        stats.push(format!("TTFT {ttft_ms} ms"));
    }
    stats.join("  ·  ")
}

fn status_badge(status: MessageStatus, colors: Colors) -> AnyElement {
    let label = match status {
        MessageStatus::Pending => "Sending",
        MessageStatus::Streaming => "Writing",
        MessageStatus::Completed => "Completed",
        MessageStatus::Stopped => "Stopped",
        MessageStatus::Failed => "Failed",
        MessageStatus::Interrupted => "Interrupted",
    };
    let danger = matches!(status, MessageStatus::Failed | MessageStatus::Interrupted);
    div()
        .rounded_full()
        .bg(if danger {
            if colors.dark {
                rgba(0xff453a24)
            } else {
                rgba(0xd7001518)
            }
        } else {
            colors.raised
        })
        .px_2()
        .py_1()
        .text_size(px(11.0))
        .text_color(if danger { colors.danger } else { colors.muted })
        .child(label)
        .into_any_element()
}

fn render_error_card(
    app: &OneChat,
    message: &Message,
    request: Option<&RequestInfo>,
    latest: bool,
    generating: bool,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> Option<AnyElement> {
    if !matches!(
        message.status,
        MessageStatus::Failed | MessageStatus::Interrupted
    ) {
        return None;
    }
    let error = request.and_then(|request| request.error.as_ref());
    let summary = error.map_or_else(
        || "Generation stopped before it completed.".to_string(),
        |error| error.message.clone(),
    );
    let detail = error
        .and_then(|error| error.detail.clone())
        .or_else(|| error.map(|error| format!("Error category: {}", error.kind)));
    let expanded = app.error_detail_expanded(&message.id);
    let retry_id = message.id.clone();
    let detail_id = message.id.clone();

    Some(
        div()
            .mt_4()
            .rounded_xl()
            .bg(if colors.dark {
                rgba(0xff453a16)
            } else {
                rgba(0xd700150d)
            })
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.danger)
                    .child(summary),
            )
            .children(expanded.then(|| {
                div().text_sm().text_color(colors.muted).child(
                    detail
                        .clone()
                        .unwrap_or_else(|| "No technical details were returned.".into()),
                )
            }))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .children((latest && !generating).then(|| {
                        primary_button(
                            SharedString::from(format!("retry-message-{}", message.id)),
                            "Try Again",
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.regenerate_assistant(retry_id.clone(), cx)
                        }))
                    }))
                    .children(detail.map(|_| {
                        button(
                            SharedString::from(format!("error-detail-{}", message.id)),
                            if expanded {
                                "Hide Details"
                            } else {
                                "Show Details"
                            },
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_error_detail(detail_id.clone(), cx)
                        }))
                    })),
            )
            .into_any_element(),
    )
}

fn render_composer(
    app: &OneChat,
    has_system_prompt: bool,
    editing_system_prompt: bool,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let generating = app.is_current_generating();
    let can_send = !app.composer.read(cx).text().trim().is_empty()
        && app.current_model().is_some()
        && app.current_conversation().is_some();
    let action = if generating {
        destructive_icon_button("composer-stop", "■", colors)
            .on_click(cx.listener(|this, _, _, cx| this.stop_current_generation(cx)))
    } else if can_send {
        primary_icon_button("composer-send", "↑", colors)
            .on_click(cx.listener(|this, _, _, cx| this.send_composer(cx)))
    } else {
        primary_icon_button("composer-send-disabled", "↑", colors)
            .opacity(0.38)
            .cursor_default()
    };

    let (previous_lines, visual_lines, height_revision) = app.composer.read(cx).height_transition();
    let previous_height = 50.0 + (previous_lines.saturating_sub(1) as f32 * 24.0);
    let target_height = 50.0 + (visual_lines.saturating_sub(1) as f32 * 24.0);
    let input = div()
        .min_w_0()
        .flex_1()
        .overflow_hidden()
        .child(app.composer.clone())
        .with_animation(
            SharedString::from(format!("composer-height-{height_revision}")),
            Animation::new(Duration::from_millis(200)).with_easing(ease_out_quint()),
            move |input, delta| {
                input.opacity(0.86 + delta * 0.14).max_h(px(
                    previous_height + (target_height - previous_height) * delta
                ))
            },
        );

    div()
        .flex_none()
        .w_full()
        .px_6()
        .pb_5()
        .child(
            div()
                .mx_auto()
                .w_full()
                .max_w(px(800.0))
                .child(
                    div()
                        .pb_2()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap_1()
                                .children((!has_system_prompt && !editing_system_prompt).then(
                                    || {
                                        compact_button(
                                            "composer-add-system-prompt",
                                            "+ System Prompt",
                                            colors,
                                        )
                                        .text_color(colors.accent)
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.begin_edit_system_prompt(cx)
                                            }),
                                        )
                                    },
                                ))
                                .children((has_system_prompt || editing_system_prompt).then(|| {
                                    compact_button("composer-system", "System Prompt", colors)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.begin_edit_system_prompt(cx)
                                        }))
                                }))
                                .child(
                                    compact_button("composer-context", "Context", colors).on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.open_inspector(InspectorTab::Context, cx)
                                        }),
                                    ),
                                )
                                .child(
                                    compact_button("composer-parameters", "Parameters", colors)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.open_inspector(InspectorTab::Model, cx)
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(colors.muted)
                                .child("↩ Send  ·  ⇧↩ New Line"),
                        ),
                )
                .child(div().flex().items_end().gap_2().child(input).child(action)),
        )
        .into_any_element()
}

fn render_system_prompt_card(
    app: &OneChat,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let conversation = app
        .current_conversation()
        .expect("system prompt card requires a conversation");
    let source = match conversation.system_prompt.source {
        SystemPromptSource::None => "None",
        SystemPromptSource::FromDefault => "Default",
        SystemPromptSource::Custom => "Custom",
    };
    let actions = match app.system_prompt_mode {
        SystemPromptMode::Compact => div()
            .flex()
            .gap_1()
            .child(
                compact_button("expand-system-prompt", "Show", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.expand_system_prompt(cx))),
            )
            .child(
                compact_button("copy-system-prompt", "Copy", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.copy_system_prompt(cx))),
            )
            .child(
                compact_button("edit-system-prompt", "Edit", colors)
                    .text_color(colors.accent)
                    .on_click(cx.listener(|this, _, _, cx| this.begin_edit_system_prompt(cx))),
            )
            .into_any_element(),
        SystemPromptMode::Expanded => div()
            .flex()
            .gap_1()
            .child(
                compact_button("collapse-system-prompt", "Hide", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.collapse_system_prompt(cx))),
            )
            .child(
                compact_button("copy-system-prompt-expanded", "Copy", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.copy_system_prompt(cx))),
            )
            .child(
                compact_button("edit-system-prompt-expanded", "Edit", colors)
                    .text_color(colors.accent)
                    .on_click(cx.listener(|this, _, _, cx| this.begin_edit_system_prompt(cx))),
            )
            .into_any_element(),
        SystemPromptMode::Editing => div()
            .flex()
            .gap_2()
            .child(
                primary_button("save-system-prompt", "Save", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.save_system_prompt(cx))),
            )
            .child(
                button("cancel-system-prompt", "Cancel", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_system_prompt_edit(cx))),
            )
            .into_any_element(),
    };
    let content = match app.system_prompt_mode {
        SystemPromptMode::Compact => div()
            .text_sm()
            .line_height(px(21.0))
            .text_color(colors.muted)
            .child(prompt_preview(&conversation.system_prompt.content))
            .into_any_element(),
        SystemPromptMode::Expanded => div()
            .text_sm()
            .line_height(px(22.0))
            .whitespace_normal()
            .child(conversation.system_prompt.content.clone())
            .into_any_element(),
        SystemPromptMode::Editing => app
            .system_prompt_editor
            .as_ref()
            .map(|editor| editor.clone().into_any_element())
            .unwrap_or_else(|| {
                div()
                    .text_sm()
                    .text_color(colors.muted)
                    .child("Opening editor…")
                    .into_any_element()
            }),
    };

    let card = div()
        .mx_auto()
        .mb_7()
        .w_full()
        .max_w(px(780.0))
        .rounded_xl()
        .bg(colors.raised)
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .size(px(22.0))
                                .rounded_lg()
                                .bg(colors.accent_soft)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(11.0))
                                .text_color(colors.accent)
                                .child("⌘"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("System Prompt"),
                        )
                        .child(
                            div()
                                .rounded_full()
                                .bg(colors.accent_soft)
                                .px_2()
                                .py_1()
                                .text_size(px(10.0))
                                .text_color(colors.accent)
                                .child(source),
                        ),
                )
                .child(actions),
        )
        .child(content);
    let animation_id = match app.system_prompt_mode {
        SystemPromptMode::Compact => "system-prompt-compact",
        SystemPromptMode::Expanded => "system-prompt-expanded",
        SystemPromptMode::Editing => "system-prompt-editing",
    };
    card.with_animation(
        animation_id,
        Animation::new(Duration::from_millis(200)).with_easing(ease_out_quint()),
        |card, delta| {
            card.opacity(0.78 + delta * 0.22)
                .mt(px(8.0 * (1.0 - delta)))
        },
    )
    .into_any_element()
}

fn thinking_preview(thinking: &str) -> String {
    const MAX_CHARACTERS: usize = 220;
    let thinking = thinking.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = thinking.chars();
    let preview = characters.by_ref().take(MAX_CHARACTERS).collect::<String>();
    if characters.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}

fn prompt_preview(prompt: &str) -> String {
    const MAX_CHARACTERS: usize = 160;
    let prompt = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = prompt.chars();
    let preview = characters.by_ref().take(MAX_CHARACTERS).collect::<String>();
    if characters.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
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
