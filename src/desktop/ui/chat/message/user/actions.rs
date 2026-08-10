use super::*;

pub(super) fn render_message_actions(
    app: &OneChat,
    turn: &Turn,
    action_group: SharedString,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let generating = app.is_current_generating();
    let editing = app.user_message_editor(turn).is_some();
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
        actions = actions.child(CopyButton::new(
            SharedString::from(format!("copy-user-message-{}", turn.id)),
            turn.user.content.clone(),
        ));
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

    div()
        .mt_1()
        .min_h(px(24.0))
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .child(branch_actions)
        .child(actions)
        .into_any_element()
}
