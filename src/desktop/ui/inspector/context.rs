use super::*;

pub(super) fn render_context(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let Some(conversation) = app.current_conversation() else {
        return notice("Select a conversation to inspect its context.", cx);
    };
    let prompt = if conversation.system_prompt.trim().is_empty() {
        "None".to_string()
    } else {
        conversation.system_prompt.clone()
    };
    let source = app.system_prompt_label(&conversation.system_prompt);
    let estimated_tokens = estimate_context_tokens(app);
    let history_preview = crate::application::generation::history_preview_for_new_turn(
        &app.data.snapshot.current_turns,
        app.displayed_history_limit(),
    );
    let request_context = app.inspected_request().and_then(|request| request.context);

    let mut content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(conversation_history_control(app, cx))
        .child(inspector_field(
            "History Preview",
            &format!(
                "{} of {} turns",
                history_preview.included_turns, history_preview.available_turns
            ),
            cx,
        ))
        .child(inspector_field("System Prompt", &prompt, cx))
        .child(inspector_field("Prompt source", &source, cx))
        .child(inspector_field(
            "Messages",
            &app.current_context_messages().len().to_string(),
            cx,
        ))
        .child(inspector_field(
            "Estimated context tokens",
            &format!("~{estimated_tokens}"),
            cx,
        ));

    if let Some(context) = request_context {
        content = content
            .child(inspector_field(
                "Last Request History",
                &format!(
                    "{} of {} turns",
                    context.included_history_turns, context.available_history_turns
                ),
                cx,
            ))
            .child(inspector_field(
                "Limited by model context window",
                if context.limited_by_context_window {
                    "Yes"
                } else {
                    "No"
                },
                cx,
            ));
    }

    content
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Regular
                        .primary_icon_action(
                            "context-edit-system-prompt",
                            AppIcon::Pencil,
                            "Edit system prompt",
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.begin_edit_system_prompt(window, cx)
                        })),
                )
                .child(
                    Regular
                        .danger_icon_action(
                            "clear-conversation-context",
                            AppIcon::Trash,
                            "Clear context",
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.request_clear_current_context(window, cx)
                        })),
                ),
        )
        .into_any_element()
}

fn conversation_history_control(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let generating = app.is_current_generating();
    let has_override = app
        .current_conversation()
        .is_some_and(|conversation| conversation.history_limit_override.is_some());

    div()
        .rounded_lg()
        .bg(cx.theme().muted)
        .p_3()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted_foreground)
                        .child("Conversation History"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .when(has_override, |actions| {
                            actions.child(
                                Button::new("reset-conversation-history-limit")
                                    .ghost()
                                    .small()
                                    .compact()
                                    .label("Reset")
                                    .tooltip("Restore the current default value")
                                    .disabled(generating)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.reset_conversation_history_limit(cx)
                                    })),
                            )
                        })
                        .child(
                            div()
                                .min_w(px(64.0))
                                .text_right()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(app.displayed_history_limit().label()),
                        ),
                ),
        )
        .child(
            Slider::new(&app.chat.history_limit_slider)
                .disabled(generating)
                .w_full()
                .bg(cx.theme().primary),
        )
        .child(
            div()
                .flex()
                .justify_between()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("No History")
                .child("Unlimited"),
        )
        .child(
            div()
                .text_size(px(11.0))
                .line_height(px(16.0))
                .text_color(cx.theme().muted_foreground)
                .child(if generating {
                    "History can be changed after the current generation finishes."
                } else {
                    "Changes apply to the next request."
                }),
        )
        .into_any_element()
}

fn estimate_context_tokens(app: &OneChat) -> u64 {
    crate::application::context_usage::estimate_input_tokens(
        app.current_conversation()
            .map_or("", |conversation| conversation.system_prompt.as_str()),
        &app.current_context_messages(),
        app.current_context_audio_duration_ms(),
    )
}
