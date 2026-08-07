use super::*;
use crate::desktop::ui::stream::should_capture_nested_scroll;
use unicode_segmentation::UnicodeSegmentation;

const USER_MESSAGE_WIDTH_RATIO: f32 = 0.75;
const USER_EDITOR_MIN_WIDTH: f32 = 160.0;
const USER_EDITOR_HORIZONTAL_CHROME: f32 = 64.0;

pub(super) fn user_editor_width(content: &str, max_width: f32) -> f32 {
    let text_width = content
        .lines()
        .map(|line| {
            line.graphemes(true)
                .map(|grapheme| if grapheme.is_ascii() { 8.0 } else { 15.0 })
                .sum::<f32>()
        })
        .fold(0.0, f32::max);
    (text_width + USER_EDITOR_HORIZONTAL_CHROME)
        .max(USER_EDITOR_MIN_WIDTH)
        .min(max_width)
}

pub(super) fn render_turn(
    app: &OneChat,
    turn: &Turn,
    message_max_width: f32,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let response = app.visible_response(turn);
    div()
        .w_full()
        .child(render_user_message(
            app,
            turn,
            message_max_width,
            colors,
            scale_factor,
            cx,
        ))
        .children(response.map(|response| {
            render_assistant_message(
                app,
                turn,
                response,
                message_max_width,
                colors,
                scale_factor,
                cx,
            )
        }))
        .into_any_element()
}

fn render_user_message(
    app: &OneChat,
    turn: &Turn,
    message_max_width: f32,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let user_message_max_width = message_max_width * USER_MESSAGE_WIDTH_RATIO;
    let generating = app.is_current_generating();
    let editor = app.user_message_editor(turn);
    let editing = editor.is_some();
    let editing_any = app.active_message_editor().is_some();
    let can_add_response = !generating
        && !editing_any
        && turn.responses.len() < 4
        && app.data.snapshot.models.iter().any(|model| {
            app.model_availability(model).is_ok()
                && !turn
                    .responses
                    .iter()
                    .any(|response| response.model_id == model.id)
        });
    let content = if let Some(editor) = editor {
        let save_id = turn.id.clone();
        let width = user_editor_width(editor.read(cx).text(), user_message_max_width);
        div()
            .w(px(width))
            .rounded_xl()
            .border_1()
            .border_color(colors.border)
            .bg(colors.panel)
            .p_3()
            .child(div().w_full().min_w_0().overflow_hidden().child(editor))
            .child(
                div()
                    .pt_3()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .child(
                        large_svg_icon_button(
                            SharedString::from(format!("cancel-edit-user-{}", turn.id)),
                            UiIcon::Close,
                            IconTone::Muted,
                            colors,
                            scale_factor,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_message_edit(cx))),
                    )
                    .child(
                        primary_svg_icon_button(
                            SharedString::from(format!("save-edit-user-{}", turn.id)),
                            UiIcon::Save,
                            colors,
                            scale_factor,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.save_user_edit(save_id.clone(), cx)
                        })),
                    ),
            )
            .into_any_element()
    } else {
        div()
            .max_w(px(user_message_max_width))
            .rounded_xl()
            .bg(colors.accent)
            .px_4()
            .py_3()
            .text_color(colors.on_accent)
            .whitespace_normal()
            .line_height(px(23.0))
            .child(SelectableText::new(
                SharedString::from(format!("user-message-content-{}", turn.user.id)),
                turn.user.content.clone(),
                app.chat.text_selection.clone(),
                rgba(0x00000038),
            ))
            .into_any_element()
    };

    let branches = app.user_branches(turn);
    let branch_index = branches
        .iter()
        .position(|branch| branch.id == turn.id)
        .unwrap_or_default();
    let previous_branch = branch_index
        .checked_sub(1)
        .and_then(|index| branches.get(index))
        .map(|turn| turn.id.clone());
    let next_branch = branches.get(branch_index + 1).map(|turn| turn.id.clone());
    let mut branch_actions = div().flex().items_center().gap_1();
    if branches.len() > 1 {
        branch_actions = branch_actions
            .children(
                (!generating && !editing_any)
                    .then_some(previous_branch)
                    .flatten()
                    .map(|branch_id| {
                        svg_icon_button(
                            SharedString::from(format!("previous-user-branch-{}", turn.id)),
                            UiIcon::ChevronLeft,
                            IconTone::Muted,
                            colors,
                            scale_factor,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_user_branch(branch_id.clone(), cx)
                        }))
                    }),
            )
            .child(
                div()
                    .px_1()
                    .text_size(px(11.0))
                    .text_color(colors.muted)
                    .child(format!("{}/{}", branch_index + 1, branches.len())),
            )
            .children(
                (!generating && !editing_any)
                    .then_some(next_branch)
                    .flatten()
                    .map(|branch_id| {
                        svg_icon_button(
                            SharedString::from(format!("next-user-branch-{}", turn.id)),
                            UiIcon::ChevronRight,
                            IconTone::Muted,
                            colors,
                            scale_factor,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_user_branch(branch_id.clone(), cx)
                        }))
                    }),
            );
    }
    let mut actions = div().flex().items_center().gap_1();
    if !editing {
        let copy_id = turn.id.clone();
        actions = actions.child(
            svg_icon_button(
                SharedString::from(format!("copy-user-message-{}", turn.id)),
                UiIcon::Copy,
                IconTone::Muted,
                colors,
                scale_factor,
            )
            .on_click(cx.listener(move |this, _, _, cx| this.copy_user(copy_id.clone(), cx))),
        );
    }
    if !generating && !editing_any {
        let edit_id = turn.id.clone();
        actions = actions.child(
            svg_icon_button(
                SharedString::from(format!("edit-user-message-{}", turn.id)),
                UiIcon::Pencil,
                IconTone::Muted,
                colors,
                scale_factor,
            )
            .on_click(cx.listener(move |this, _, _, cx| this.begin_edit_user(edit_id.clone(), cx))),
        );
    }
    if can_add_response {
        let turn_id = turn.id.clone();
        actions = actions.child(
            svg_icon_button(
                SharedString::from(format!("add-response-{}", turn.id)),
                UiIcon::At,
                IconTone::Muted,
                colors,
                scale_factor,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_response_model_picker(turn_id.clone(), cx)
            })),
        );
    }

    let action_bar = div()
        .mt_1()
        .min_h(px(24.0))
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .child(branch_actions)
        .child(actions);

    div()
        .mx_auto()
        .mb_7()
        .w_full()
        .max_w(px(message_max_width))
        .flex()
        .justify_end()
        .child(
            div()
                .max_w(px(user_message_max_width))
                .min_w_0()
                .flex()
                .flex_col()
                .items_end()
                .child(content)
                .child(action_bar),
        )
        .into_any_element()
}

fn render_assistant_message(
    app: &OneChat,
    turn: &Turn,
    message: &AssistantResponse,
    message_max_width: f32,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let request = app.request_for_response(message);
    let assistant_label = format!("{} · {}", message.model_name, message.provider_name);
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
                        large_svg_icon_button(
                            SharedString::from(format!("cancel-edit-message-{}", message.id)),
                            UiIcon::Close,
                            IconTone::Muted,
                            colors,
                            scale_factor,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_message_edit(cx))),
                    )
                    .child(
                        primary_svg_icon_button(
                            SharedString::from(format!("save-edit-message-{}", message.id)),
                            UiIcon::Save,
                            colors,
                            scale_factor,
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
        markdown::render(
            document,
            &message.id,
            &app.chat.text_selection,
            colors,
            scale_factor,
        )
    } else {
        markdown::render_plain(
            &message.content,
            &message.id,
            &app.chat.text_selection,
            colors,
        )
    };

    let latest = app.is_latest_turn(&turn.id);
    let generating = app.is_current_generating();
    let copy_id = message.id.clone();
    let edit_id = message.id.clone();
    let regenerate_id = message.id.clone();
    let info_id = message.id.clone();
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
    if latest && !generating && (!editing_any || editing) {
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
    if latest
        && !generating
        && message.status == MessageStatus::Completed
        && !message.content.is_empty()
        && turn.continuation_response_id.as_deref() != Some(&message.id)
    {
        let context_turn_id = turn.id.clone();
        let context_response_id = message.id.clone();
        actions = actions.child(
            svg_icon_button(
                SharedString::from(format!("use-response-context-{}", message.id)),
                UiIcon::Context,
                IconTone::Muted,
                colors,
                scale_factor,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.use_response_for_context(
                    context_turn_id.clone(),
                    context_response_id.clone(),
                    cx,
                )
            })),
        );
    }

    let header = if turn.responses.len() > 1 {
        let mut tabs = div().mb_3().flex().flex_wrap().items_center().gap_1();
        for response in &turn.responses {
            let selected = response.id == message.id;
            let context = turn.continuation_response_id.as_deref() == Some(&response.id);
            let status = match response.status {
                MessageStatus::Pending | MessageStatus::Streaming => "  ·  …",
                MessageStatus::Failed | MessageStatus::Interrupted => "  ·  !",
                MessageStatus::Stopped => "  ·  ■",
                MessageStatus::Completed => "",
            };
            let label = format!("{}{}", response.model_name, status);
            let tab_turn_id = turn.id.clone();
            let tab_response_id = response.id.clone();
            tabs = tabs.child(
                compact_button(
                    SharedString::from(format!("response-tab-{}", response.id)),
                    label,
                    colors,
                )
                .flex()
                .items_center()
                .gap_1()
                .children(context.then(|| {
                    svg_icon(
                        UiIcon::Context,
                        IconTone::Accent,
                        colors,
                        scale_factor,
                        13.0,
                    )
                }))
                .bg(if selected {
                    colors.accent_soft
                } else {
                    rgba(0x00000000)
                })
                .text_color(if selected {
                    colors.accent
                } else {
                    colors.muted
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.show_response(tab_turn_id.clone(), tab_response_id.clone(), cx)
                })),
            );
        }
        tabs.into_any_element()
    } else {
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
                    .child(assistant_label),
            )
            .children(
                (!matches!(message.status, MessageStatus::Completed))
                    .then(|| status_badge(message.status, colors)),
            )
            .into_any_element()
    };
    let stats = request.map(format_message_stats).unwrap_or_default();
    div()
        .id(SharedString::from(format!(
            "assistant-message-{}",
            message.id
        )))
        .mx_auto()
        .mb_8()
        .w_full()
        .max_w(px(message_max_width))
        .child(header)
        .children(render_reasoning(
            app,
            message,
            request,
            colors,
            scale_factor,
            cx,
        ))
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

fn render_reasoning(
    app: &OneChat,
    message: &AssistantResponse,
    request: Option<&RequestInfo>,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> Option<AnyElement> {
    if message.thinking.is_empty() {
        return None;
    }

    let streaming = matches!(
        message.status,
        MessageStatus::Pending | MessageStatus::Streaming
    );
    let live = streaming && request.is_some_and(|request| request.thinking_duration_ms.is_none());
    let expanded = app.thinking_expanded(&message.id, live);
    let duration = request
        .and_then(|request| reasoning_duration_ms(app, request, live))
        .map(format_reasoning_duration);

    let mut controls = div().flex().items_center().gap_2();
    if let Some(duration) = duration {
        controls = controls.child(
            div()
                .rounded_full()
                .bg(colors.panel)
                .px_2()
                .py_1()
                .flex()
                .items_center()
                .gap_1()
                .text_size(px(10.0))
                .text_color(if live { colors.accent } else { colors.muted })
                .children(live.then(|| div().size(px(5.0)).rounded_full().bg(colors.accent)))
                .child(duration),
        );
    }
    let thinking_id = message.id.clone();
    controls = controls.child(
        svg_icon_button(
            SharedString::from(format!("thinking-{}", message.id)),
            if expanded {
                UiIcon::ChevronUp
            } else {
                UiIcon::ChevronDown
            },
            IconTone::Accent,
            colors,
            scale_factor,
        )
        .on_click(
            cx.listener(move |this, _, _, cx| this.toggle_thinking(thinking_id.clone(), live, cx)),
        )
        .with_animation(
            SharedString::from(format!(
                "thinking-toggle-{}-{}",
                if expanded { "expanded" } else { "collapsed" },
                message.id
            )),
            Animation::new(Duration::from_millis(180)).with_easing(ease_out_quint()),
            |button, delta| button.opacity(0.7 + delta * 0.3),
        ),
    );

    let scroll = app.chat.thinking_scrolls.get(&message.id).cloned();
    let boundary_scroll = scroll.clone();
    let body = div()
        .id(SharedString::from(format!(
            "thinking-content-{}",
            message.id
        )))
        .whitespace_normal()
        .overflow_y_scroll()
        .pr_2()
        .child(SelectableText::new(
            SharedString::from(format!("thinking-text-{}", message.id)),
            message.thinking.clone(),
            app.chat.text_selection.clone(),
            selection_color(colors.dark),
        ));
    let body = if let Some(scroll) = scroll.as_ref() {
        body.track_scroll(scroll)
    } else {
        body
    };
    let body = if let Some(motion) = app.chat.thinking_motions.get(&message.id).copied() {
        let target_height = if expanded {
            motion.full_height
        } else {
            motion.full_height.min(COLLAPSED_THINKING_HEIGHT)
        };
        let animation_id = SharedString::from(format!(
            "thinking-{}-{}",
            if expanded { "expand" } else { "collapse" },
            message.id
        ));
        body.with_animation(
            animation_id,
            Animation::new(Duration::from_millis(220)).with_easing(ease_out_quint()),
            move |body, delta| {
                if delta < 1.0
                    && let Some(scroll) = scroll.as_ref()
                {
                    scroll.scroll_to_bottom();
                }
                if expanded && delta >= 1.0 {
                    body
                } else {
                    body.max_h(px(
                        motion.from_height + (target_height - motion.from_height) * delta
                    ))
                }
            },
        )
        .into_any_element()
    } else if expanded {
        body.into_any_element()
    } else {
        body.max_h(px(COLLAPSED_THINKING_HEIGHT)).into_any_element()
    };
    let body = if expanded {
        body
    } else {
        div()
            .id(SharedString::from(format!(
                "thinking-scroll-boundary-{}",
                message.id
            )))
            .on_scroll_wheel(move |event, window, cx| {
                let Some(scroll) = boundary_scroll.as_ref() else {
                    return;
                };
                let delta_y = event.delta.pixel_delta(window.line_height()).y;
                // GPUI scrolls the child before this ancestor listener runs.
                let offset_before_event = scroll.offset().y - delta_y;
                if should_capture_nested_scroll(
                    f32::from(delta_y),
                    f32::from(offset_before_event),
                    f32::from(scroll.max_offset().height),
                ) {
                    cx.stop_propagation();
                }
            })
            .child(body)
            .into_any_element()
    };

    Some(
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
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("REASONING"),
                    )
                    .child(controls),
            )
            .child(body)
            .into_any_element(),
    )
}

fn reasoning_duration_ms(app: &OneChat, request: &RequestInfo, live: bool) -> Option<u64> {
    if let Some(duration) = request.thinking_duration_ms {
        return Some(duration);
    }
    if !live {
        return None;
    }
    app.chat
        .thinking_started_at
        .get(&request.id)
        .map(|started_at| started_at.elapsed().as_millis() as u64)
}

pub(super) fn format_reasoning_duration(duration_ms: u64) -> String {
    format!("{}.{:01}s", duration_ms / 1_000, duration_ms % 1_000 / 100)
}

pub(super) fn format_message_stats(request: &RequestInfo) -> String {
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
    message: &AssistantResponse,
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
