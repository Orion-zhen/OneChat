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

    div()
        .flex()
        .flex_col()
        .gap_3()
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
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    primary_icon_action(
                        "context-edit-system-prompt",
                        AppIcon::Pencil,
                        "Edit system prompt",
                        cx,
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| {
                            this.begin_edit_system_prompt(window, cx)
                        }),
                    ),
                )
                .child(
                    danger_icon_action(
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

fn estimate_context_tokens(app: &OneChat) -> usize {
    let characters = app
        .current_conversation()
        .map(|conversation| conversation.system_prompt.chars().count())
        .unwrap_or_default()
        + app
            .current_context_messages()
            .iter()
            .map(|message| serde_json::to_string(message).map_or(0, |value| value.chars().count()))
            .sum::<usize>();
    characters.div_ceil(4)
}
