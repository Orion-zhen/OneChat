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
        let add_id = turn.id.clone();
        let editor_input = editor.input.clone();
        let attachment_count = editor.attachments.len() + editor.attachment_drafts.len();
        let attachments_loading = editor.attachment_load_id.is_some();
        let has_attachments = attachment_count > 0 || attachments_loading;
        let width = if has_attachments {
            user_message_max_width
        } else {
            user_editor_width(
                &editor_input.read(cx).value(),
                user_message_max_width,
                typography.body_size,
            )
        };
        div()
            .w(px(width))
            .rounded(px(18.0))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .p_3()
            .children(has_attachments.then(|| {
                div()
                    .id(SharedString::from(format!(
                        "edit-user-attachments-{}",
                        turn.id
                    )))
                    .w_full()
                    .min_w_0()
                    .overflow_x_scroll()
                    .restrict_scroll_to_axis()
                    .pb_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .children(editor.attachments.iter().map(|attachment| {
                        render_edit_stored_attachment(app, &turn.id, attachment, cx)
                    }))
                    .children(editor.attachment_drafts.iter().map(|attachment| {
                        render_edit_draft_attachment(
                            &turn.id,
                            attachment,
                            editor.attachment_previews.get(&attachment.id).cloned(),
                            cx,
                        )
                    }))
                    .children(attachments_loading.then(|| render_edit_attachment_loading(cx)))
            }))
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
                        Input::new(&editor_input)
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
                    .justify_between()
                    .gap_2()
                    .child(
                        Button::new(SharedString::from(format!(
                            "add-edit-user-attachment-{}",
                            turn.id
                        )))
                        .ghost()
                        .rounded(px(18.0))
                        .tooltip("Add attachment")
                        .size(px(36.0))
                        .p_0()
                        .disabled(
                            attachments_loading
                                || attachment_count
                                    >= crate::application::attachments::MAX_ATTACHMENTS,
                        )
                        .child(render_icon(AppIcon::Plus, IconTone::Muted, 20.0, cx))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.add_message_edit_attachments(add_id.clone(), cx)
                        })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
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
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.cancel_message_edit(cx)),
                                ),
                            )
                            .child(
                                primary_icon_button(
                                    SharedString::from(format!("save-edit-user-{}", turn.id)),
                                    AppIcon::Save,
                                    cx,
                                )
                                .disabled(attachments_loading)
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.save_user_edit(save_on_mouse_down_id.clone(), cx);
                                        cx.stop_propagation();
                                    }),
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| this.save_user_edit(save_id.clone(), cx),
                                )),
                            ),
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

fn render_edit_stored_attachment(
    app: &OneChat,
    turn_id: &str,
    attachment: &crate::domain::Attachment,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let visual = match attachment.kind {
        crate::domain::AttachmentKind::Text => edit_attachment_icon(cx),
        crate::domain::AttachmentKind::Image | crate::domain::AttachmentKind::Pdf => attachment
            .files
            .first()
            .and_then(|file| app.attachment_file_path(file))
            .map(|path| {
                img(path)
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            })
            .unwrap_or_else(|| edit_attachment_icon(cx)),
    };
    edit_attachment_card(
        turn_id,
        &attachment.id,
        &attachment.name,
        edit_attachment_detail(attachment.kind, attachment.files.len()),
        visual,
        cx,
    )
}

fn render_edit_draft_attachment(
    turn_id: &str,
    attachment: &crate::domain::AttachmentDraft,
    preview: Option<std::sync::Arc<gpui::Image>>,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let visual = preview
        .map(|preview| {
            img(preview)
                .size_full()
                .object_fit(ObjectFit::Contain)
                .into_any_element()
        })
        .unwrap_or_else(|| edit_attachment_icon(cx));
    edit_attachment_card(
        turn_id,
        &attachment.id,
        &attachment.name,
        edit_attachment_detail(attachment.kind, attachment.files.len()),
        visual,
        cx,
    )
}

fn edit_attachment_detail(kind: crate::domain::AttachmentKind, file_count: usize) -> String {
    match kind {
        crate::domain::AttachmentKind::Text => "Text document".into(),
        crate::domain::AttachmentKind::Image => "Image".into(),
        crate::domain::AttachmentKind::Pdf => format!(
            "PDF · {file_count} page{}",
            if file_count == 1 { "" } else { "s" }
        ),
    }
}

fn edit_attachment_icon(cx: &App) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(render_icon(AppIcon::FileText, IconTone::Accent, 21.0, cx))
        .into_any_element()
}

fn edit_attachment_card(
    turn_id: &str,
    attachment_id: &str,
    name: &str,
    detail: String,
    visual: AnyElement,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let remove_turn_id = turn_id.to_string();
    let remove_attachment_id = attachment_id.to_string();
    div()
        .relative()
        .w(px(196.0))
        .h(px(68.0))
        .flex_none()
        .rounded(px(14.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .p_2()
        .pr_8()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .size(px(44.0))
                .flex_none()
                .overflow_hidden()
                .rounded(px(11.0))
                .bg(cx.theme().accent)
                .child(visual),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .min_w_0()
                        .text_size(px(12.0))
                        .line_height(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(name.to_string()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .line_height(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(detail),
                ),
        )
        .child(
            Button::new(SharedString::from(format!(
                "remove-edit-attachment-{turn_id}-{attachment_id}"
            )))
            .ghost()
            .tooltip("Remove attachment")
            .size(px(24.0))
            .p_0()
            .rounded(px(12.0))
            .absolute()
            .top(px(5.0))
            .right(px(5.0))
            .child(render_icon(AppIcon::Close, IconTone::Foreground, 12.0, cx))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.remove_message_edit_attachment(
                    remove_turn_id.clone(),
                    remove_attachment_id.clone(),
                    cx,
                )
            })),
        )
        .into_any_element()
}

fn render_edit_attachment_loading(cx: &App) -> AnyElement {
    div()
        .w(px(108.0))
        .h(px(68.0))
        .flex_none()
        .rounded(px(14.0))
        .bg(cx.theme().muted)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .text_color(cx.theme().muted_foreground)
        .child("Preparing…")
        .into_any_element()
}
