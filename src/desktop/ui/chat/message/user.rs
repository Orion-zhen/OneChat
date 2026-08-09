use super::*;

pub(in crate::desktop::ui::chat) fn render_user_turn(
    app: &OneChat,
    turn: &Turn,
    message_max_width: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    animated_message(
        render_user_message(app, turn, message_max_width, typography, cx),
        format!("user-{}", turn.id),
    )
}

fn render_user_message(
    app: &OneChat,
    turn: &Turn,
    message_max_width: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let user_message_max_width = message_max_width * USER_MESSAGE_WIDTH_RATIO;
    let action_group: SharedString = format!("user-actions-{}", turn.id).into();
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
        let save_on_mouse_down_id = save_id.clone();
        let width = user_editor_width(
            &editor.read(cx).value(),
            user_message_max_width,
            typography.body_size,
        );
        div()
            .w(px(width))
            .rounded(px(18.0))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .p_3()
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_move(|_, _, cx| cx.stop_propagation())
                    .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_up_out(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_action(
                        cx.listener(|this, _: &InputEscape, _, cx| this.cancel_message_edit(cx)),
                    )
                    .child(
                        Input::new(&editor)
                            .aria_label("Edit user message")
                            .bg(cx.theme().muted)
                            .text_size(px(typography.body_size))
                            .line_height(px(typography.body_line_height)),
                    ),
            )
            .child(
                div()
                    .pt_3()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .child(
                        large_icon_button(
                            SharedString::from(format!("cancel-edit-user-{}", turn.id)),
                            AppIcon::Close,
                            IconTone::Muted,
                            cx,
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.cancel_message_edit(cx);
                                cx.stop_propagation();
                            }),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_message_edit(cx))),
                    )
                    .child(
                        primary_icon_button(
                            SharedString::from(format!("save-edit-user-{}", turn.id)),
                            AppIcon::Save,
                            cx,
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.save_user_edit(save_on_mouse_down_id.clone(), cx);
                                cx.stop_propagation();
                            }),
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
            .min_w_0()
            .flex()
            .flex_col()
            .items_end()
            .gap_2()
            .children(turn.user.attachments.iter().map(|attachment| {
                render_sent_attachment(app, attachment, user_message_max_width, cx)
            }))
            .children((!turn.user.content.is_empty()).then(|| {
                div()
                    .max_w(px(user_message_max_width))
                    .rounded(px(18.0))
                    .bg(cx.theme().primary)
                    .px_4()
                    .py_3()
                    .text_color(cx.theme().primary_foreground)
                    .whitespace_normal()
                    .text_size(px(typography.body_size))
                    .line_height(px(typography.body_line_height))
                    .child(SelectableText::new(
                        SharedString::from(format!("user-message-content-{}", turn.user.id)),
                        turn.user.content.clone(),
                        app.chat.text_selection.clone(),
                        rgba(0x00000038),
                    ))
            }))
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
                        icon_button(
                            SharedString::from(format!("previous-user-branch-{}", turn.id)),
                            AppIcon::ChevronLeft,
                            IconTone::Muted,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_user_branch(branch_id.clone(), cx)
                        }))
                    }),
            )
            .child(
                div()
                    .px_1()
                    .text_size(px(typography.micro_size))
                    .line_height(px(typography.micro_line_height))
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{}/{}", branch_index + 1, branches.len())),
            )
            .children(
                (!generating && !editing_any)
                    .then_some(next_branch)
                    .flatten()
                    .map(|branch_id| {
                        icon_button(
                            SharedString::from(format!("next-user-branch-{}", turn.id)),
                            AppIcon::ChevronRight,
                            IconTone::Muted,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_user_branch(branch_id.clone(), cx)
                        }))
                    }),
            );
    }
    let mut actions = div()
        .invisible()
        .group_hover(action_group.clone(), |actions| actions.visible())
        .flex()
        .items_center()
        .gap_1();
    if !editing {
        let copy_id = turn.id.clone();
        actions = actions.child(
            icon_button(
                SharedString::from(format!("copy-user-message-{}", turn.id)),
                AppIcon::Copy,
                IconTone::Muted,
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| this.copy_user(copy_id.clone(), cx))),
        );
    }
    if !generating && !editing_any {
        let edit_id = turn.id.clone();
        actions = actions.child(
            icon_button(
                SharedString::from(format!("edit-user-message-{}", turn.id)),
                AppIcon::Pencil,
                IconTone::Muted,
                cx,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.begin_edit_user(edit_id.clone(), window, cx)
            })),
        );
    }
    if can_add_response {
        let turn_id = turn.id.clone();
        actions = actions.child(
            icon_button(
                SharedString::from(format!("add-response-{}", turn.id)),
                AppIcon::At,
                IconTone::Muted,
                cx,
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_response_model_picker(turn_id.clone(), window, cx)
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
                .group(action_group)
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
