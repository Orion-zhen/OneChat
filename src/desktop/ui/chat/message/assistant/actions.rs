use super::*;

pub(super) fn render_message_actions(
    app: &OneChat,
    turn: &Turn,
    message: &AssistantResponse,
    latest: bool,
    generating: bool,
    action_group: SharedString,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let editing = app.assistant_message_editor(message).is_some();
    let editing_any = app.active_message_editor().is_some();
    let has_info = app.request_for_response(message).is_some();
    let has_content = !message.content.is_empty();
    let can_copy = has_content;
    let can_edit = !generating && (!editing_any || editing);
    let can_regenerate = latest
        && !generating
        && !editing
        && !matches!(
            message.status,
            MessageStatus::Failed | MessageStatus::Interrupted
        );
    let can_use_context = !generating
        && message.status == MessageStatus::Completed
        && has_content
        && turn.continuation_response_id.as_deref() != Some(&message.id);
    let can_fork = !editing_any && message.status == MessageStatus::Completed && has_content;

    let content_actions = if can_copy || can_edit {
        let mut group = div().flex().items_center().gap_1();
        if can_copy {
            group = group.child(CopyButton::new(
                SharedString::from(format!("copy-message-{}", message.id)),
                message.content.clone(),
            ));
        }
        if can_edit {
            let edit_id = message.id.clone();
            group = group.child(
                icon_button(
                    SharedString::from(format!("edit-message-{}", message.id)),
                    AppIcon::Pencil,
                    if editing {
                        IconTone::Accent
                    } else {
                        IconTone::Muted
                    },
                    cx,
                )
                .selected(editing)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.begin_edit_assistant(edit_id.clone(), window, cx)
                })),
            );
        }
        Some(group)
    } else {
        None
    };

    let response_actions = if can_regenerate || can_use_context {
        let mut group = div().flex().items_center().gap_1();
        if can_regenerate {
            let regenerate_id = message.id.clone();
            group = group.child(
                icon_button(
                    SharedString::from(format!("regenerate-message-{}", message.id)),
                    AppIcon::Regenerate,
                    IconTone::Muted,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.regenerate_assistant(regenerate_id.clone(), cx)
                })),
            );
        }
        if can_use_context {
            let context_turn_id = turn.id.clone();
            let context_response_id = message.id.clone();
            group = group.child(
                icon_button(
                    SharedString::from(format!("use-response-context-{}", message.id)),
                    AppIcon::ContextSelect,
                    IconTone::Muted,
                    cx,
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
        Some(group)
    } else {
        None
    };

    let conversation_actions = if can_fork {
        let fork_id = message.id.clone();
        Some(
            div().flex().items_center().gap_1().child(
                icon_button(
                    SharedString::from(format!("fork-message-{}", message.id)),
                    AppIcon::Fork,
                    IconTone::Muted,
                    cx,
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.fork_from_response(fork_id.clone(), cx)),
                ),
            ),
        )
    } else {
        None
    };

    let info_actions = if has_info {
        let info_id = message.id.clone();
        Some(
            div().flex().items_center().gap_1().child(
                icon_button(
                    SharedString::from(format!("info-message-{}", message.id)),
                    AppIcon::Info,
                    IconTone::Muted,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.inspect_message_request(info_id.clone(), cx)
                })),
            ),
        )
    } else {
        None
    };

    div()
        .invisible()
        .group_hover(action_group.clone(), |actions| actions.visible())
        .flex()
        .items_center()
        .gap_2()
        .children(content_actions)
        .children(response_actions)
        .children(conversation_actions)
        .children(info_actions)
        .into_any_element()
}
