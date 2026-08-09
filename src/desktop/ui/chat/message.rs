use super::*;
use crate::desktop::ui::stream::should_capture_nested_scroll;
use unicode_segmentation::UnicodeSegmentation;

const USER_MESSAGE_WIDTH_RATIO: f32 = 0.75;
const USER_EDITOR_MIN_WIDTH: f32 = 160.0;
const USER_EDITOR_HORIZONTAL_CHROME: f32 = 64.0;
const USER_EDITOR_MEASUREMENT_FONT_SIZE: f32 = 15.0;
const SENT_IMAGE_MAX_WIDTH: f32 = 520.0;
const SENT_IMAGE_MAX_HEIGHT: f32 = 360.0;

pub(super) fn user_editor_width(content: &str, max_width: f32, font_size: f32) -> f32 {
    let text_scale = font_size / USER_EDITOR_MEASUREMENT_FONT_SIZE;
    let text_width = content
        .lines()
        .map(|line| {
            line.graphemes(true)
                .map(|grapheme| (if grapheme.is_ascii() { 8.0 } else { 15.0 }) * text_scale)
                .sum::<f32>()
        })
        .fold(0.0, f32::max);
    (text_width + USER_EDITOR_HORIZONTAL_CHROME)
        .max(USER_EDITOR_MIN_WIDTH)
        .min(max_width)
}

fn render_sent_attachment(
    app: &OneChat,
    attachment: &crate::domain::Attachment,
    max_width: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    match attachment.kind {
        crate::domain::AttachmentKind::Image => render_sent_image(app, attachment, max_width, cx),
        crate::domain::AttachmentKind::Text | crate::domain::AttachmentKind::Pdf => {
            render_sent_file(app, attachment, max_width, cx)
        }
    }
}

fn sent_image_size(path: &std::path::Path, max_width: f32) -> (f32, f32) {
    static DIMENSIONS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, (u32, u32)>>,
    > = std::sync::OnceLock::new();

    let dimensions = DIMENSIONS.get_or_init(Default::default);
    let (source_width, source_height) = *dimensions
        .lock()
        .expect("attachment dimension cache poisoned")
        .entry(path.to_path_buf())
        .or_insert_with(|| image::image_dimensions(path).unwrap_or((320, 200)));
    let source_width = source_width.max(1) as f32;
    let source_height = source_height.max(1) as f32;
    let scale = (max_width.min(SENT_IMAGE_MAX_WIDTH) / source_width)
        .min(SENT_IMAGE_MAX_HEIGHT / source_height)
        .min(1.0);
    (source_width * scale, source_height * scale)
}

fn render_sent_image(
    app: &OneChat,
    attachment: &crate::domain::Attachment,
    max_width: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let Some(path) = attachment
        .files
        .first()
        .and_then(|file| app.attachment_file_path(file))
    else {
        return sent_attachment_fallback(attachment, max_width, cx);
    };
    let (width, height) = sent_image_size(&path, max_width);
    let fallback_name = attachment.name.clone();
    let muted = cx.theme().muted;
    let muted_foreground = cx.theme().muted_foreground;

    div()
        .id(SharedString::from(format!(
            "user-attachment-image-{}",
            attachment.id
        )))
        .w(px(width))
        .h(px(height))
        .flex_none()
        .overflow_hidden()
        .rounded(px(16.0))
        .border_1()
        .border_color(if cx.theme().is_dark() {
            rgba(0xffffff26)
        } else {
            rgba(0x0000001f)
        })
        .bg(muted)
        .shadow_xs()
        .child(
            img(path)
                .size_full()
                .rounded(px(15.0))
                .object_fit(ObjectFit::Contain)
                .with_fallback(move || {
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.0))
                        .text_color(muted_foreground)
                        .child(format!("Could not preview {fallback_name}"))
                        .into_any_element()
                }),
        )
        .into_any_element()
}

fn attachment_icon(cx: &App) -> AnyElement {
    div()
        .size(px(44.0))
        .flex_none()
        .rounded(px(12.0))
        .bg(cx.theme().accent)
        .flex()
        .items_center()
        .justify_center()
        .child(render_icon(AppIcon::FileText, IconTone::Accent, 21.0, cx))
        .into_any_element()
}

fn sent_file_card(
    attachment: &crate::domain::Attachment,
    detail: String,
    visual: AnyElement,
    max_width: f32,
    cx: &App,
) -> AnyElement {
    div()
        .w(px(260.0_f32.min(max_width)))
        .min_h(px(68.0))
        .rounded(px(16.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .shadow_xs()
        .p_2()
        .flex()
        .items_center()
        .gap_3()
        .text_color(cx.theme().foreground)
        .child(visual)
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
                        .child(attachment.name.clone()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .line_height(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(detail),
                ),
        )
        .into_any_element()
}

fn render_sent_file(
    app: &OneChat,
    attachment: &crate::domain::Attachment,
    max_width: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let is_pdf = attachment.kind == crate::domain::AttachmentKind::Pdf;
    let detail = if is_pdf {
        format!(
            "PDF · {} page{}",
            attachment.files.len(),
            if attachment.files.len() == 1 { "" } else { "s" }
        )
    } else {
        "Text document".to_string()
    };
    let visual = if is_pdf {
        attachment
            .files
            .first()
            .and_then(|file| app.attachment_file_path(file))
            .map(|path| {
                div()
                    .w(px(42.0))
                    .h(px(52.0))
                    .flex_none()
                    .overflow_hidden()
                    .rounded(px(9.0))
                    .border_1()
                    .border_color(rgba(0x0000001f))
                    .bg(rgba(0xffffffff))
                    .shadow_xs()
                    .child(img(path).size_full().object_fit(ObjectFit::Contain))
                    .into_any_element()
            })
            .unwrap_or_else(|| attachment_icon(cx))
    } else {
        attachment_icon(cx)
    };

    sent_file_card(attachment, detail, visual, max_width, cx)
}

fn sent_attachment_fallback(
    attachment: &crate::domain::Attachment,
    max_width: f32,
    cx: &App,
) -> AnyElement {
    sent_file_card(
        attachment,
        "Attachment unavailable".into(),
        attachment_icon(cx),
        max_width,
        cx,
    )
}

pub(super) fn render_user_turn(
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

pub(super) fn render_assistant_turn(
    app: &OneChat,
    turn: &Turn,
    response: &AssistantResponse,
    message_max_width: f32,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    animated_message(
        render_assistant_message(
            app,
            turn,
            response,
            message_max_width,
            scale_factor,
            typography,
            cx,
        ),
        format!("assistant-{}", response.id),
    )
}

fn animated_message(message: AnyElement, id: String) -> AnyElement {
    div()
        .id(SharedString::from(format!("message-anchor-{id}")))
        .relative()
        .w_full()
        .child(message)
        .with_animation(
            SharedString::from(format!("message-appear-{id}")),
            Animation::new(Duration::from_millis(240)).with_easing(ease_out_quint()),
            |message, delta| {
                message
                    .opacity(0.72 + delta * 0.28)
                    .top(px(6.0 * (1.0 - delta)))
            },
        )
        .into_any_element()
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

fn render_assistant_message(
    app: &OneChat,
    turn: &Turn,
    message: &AssistantResponse,
    message_max_width: f32,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let request = app.request_for_response(message);
    let action_group: SharedString = format!("assistant-actions-{}", message.id).into();
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
        let save_on_mouse_down_id = save_id.clone();
        div()
            .rounded_xl()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .p_3()
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_move(|_, _, cx| cx.stop_propagation())
                    .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_up_out(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_action(
                        cx.listener(|this, _: &InputEscape, _, cx| this.cancel_message_edit(cx)),
                    )
                    .child(
                        Input::new(&editor)
                            .aria_label("Edit assistant response")
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
                            SharedString::from(format!("cancel-edit-message-{}", message.id)),
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
                            SharedString::from(format!("save-edit-message-{}", message.id)),
                            AppIcon::Save,
                            cx,
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.save_assistant_edit(save_on_mouse_down_id.clone(), cx);
                                cx.stop_propagation();
                            }),
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
            .text_size(px(typography.metadata_size))
            .line_height(px(typography.metadata_line_height))
            .text_color(cx.theme().muted_foreground)
            .child(div().size(px(7.0)).rounded_full().bg(cx.theme().primary))
            .child(waiting_label(message))
            .into_any_element()
    } else if let Some(document) = app.markdown_for(message) {
        markdown::render(
            document,
            &message.id,
            &app.chat.text_selection,
            scale_factor,
            typography,
            cx,
        )
    } else {
        markdown::render_plain(
            &message.content,
            &message.id,
            &app.chat.text_selection,
            typography,
            cx,
        )
    };

    let latest = app.is_latest_turn(&turn.id);
    let generating = app.is_current_generating();
    let has_content = !message.content.is_empty();
    let can_copy = has_content;
    let can_edit = latest && !generating && (!editing_any || editing);
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
    let has_info = request.is_some();

    let content_actions = if can_copy || can_edit {
        let mut group = div().flex().items_center().gap_1();
        if can_copy {
            let copy_id = message.id.clone();
            group = group.child(
                icon_button(
                    SharedString::from(format!("copy-message-{}", message.id)),
                    AppIcon::Copy,
                    IconTone::Muted,
                    cx,
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.copy_assistant(copy_id.clone(), cx)),
                ),
            );
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

    let actions = div()
        .invisible()
        .group_hover(action_group.clone(), |actions| actions.visible())
        .flex()
        .items_center()
        .gap_2()
        .children(content_actions)
        .children(response_actions)
        .children(conversation_actions)
        .children(info_actions);

    let multiple_responses = turn.responses.len() > 1;
    let header_content = if multiple_responses {
        let mut tabs = div()
            .id(SharedString::from(format!("response-tabs-{}", turn.id)))
            .min_w_0()
            .flex_1()
            .flex()
            .items_center()
            .gap_1()
            .overflow_x_scroll()
            .restrict_scroll_to_axis();
        for response in &turn.responses {
            let selected = response.id == message.id;
            let context = turn.continuation_response_id.as_deref() == Some(&response.id);
            let status = match response.status {
                MessageStatus::Pending | MessageStatus::Streaming => "  ·  …",
                MessageStatus::Failed | MessageStatus::Interrupted => "  ·  !",
                MessageStatus::Stopped => "  ·  ■",
                MessageStatus::Completed => "",
            };
            let label = format!(
                "{} · {}{}",
                response.model_name, response.provider_name, status
            );
            let tab_turn_id = turn.id.clone();
            let tab_response_id = response.id.clone();
            tabs = tabs.child(
                response_tab_button(
                    SharedString::from(format!("response-tab-{}", response.id)),
                    label,
                    typography,
                )
                .selected(selected)
                .flex()
                .items_center()
                .gap_1()
                .children(
                    context
                        .then(|| render_icon(AppIcon::ContextSelected, IconTone::Accent, 15.0, cx)),
                )
                .bg(if selected {
                    cx.theme().accent
                } else {
                    cx.theme().transparent
                })
                .text_color(if selected {
                    cx.theme().primary
                } else {
                    cx.theme().muted_foreground
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.show_response(tab_turn_id.clone(), tab_response_id.clone(), cx)
                })),
            );
        }
        tabs.into_any_element()
    } else {
        div()
            .text_size(px(typography.metadata_size))
            .line_height(px(typography.metadata_line_height))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(cx.theme().muted_foreground)
            .child(assistant_label)
            .into_any_element()
    };
    let header = div()
        .mb_3()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .size(px(24.0))
                .flex_none()
                .rounded_lg()
                .bg(cx.theme().accent)
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().primary)
                .child(render_icon(AppIcon::Sparkles, IconTone::Accent, 13.0, cx)),
        )
        .child(header_content)
        .children(
            (!multiple_responses && !matches!(message.status, MessageStatus::Completed))
                .then(|| status_badge(message.status, typography, cx)),
        );
    let stats = request.map(format_message_stats).unwrap_or_default();
    div()
        .id(SharedString::from(format!(
            "assistant-message-{}",
            message.id
        )))
        .mx_auto()
        .group(action_group)
        .mb_8()
        .w_full()
        .max_w(px(message_max_width))
        .child(header)
        .children(render_reasoning(app, message, request, typography, cx))
        .children(render_tool_executions(app, message, typography, cx))
        .child(content)
        .children(render_error_card(
            app, message, request, latest, generating, typography, cx,
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
                        .text_size(px(typography.micro_size))
                        .line_height(px(typography.micro_line_height))
                        .text_color(cx.theme().muted_foreground)
                        .child(stats)
                })),
        )
        .into_any_element()
}

pub(super) fn waiting_label(message: &AssistantResponse) -> String {
    if let Some(execution) = message
        .tool_executions
        .iter()
        .rev()
        .find(|execution| execution.status.is_active())
    {
        let action = match execution.status {
            ToolExecutionStatus::Queued => "Preparing",
            ToolExecutionStatus::Running => "Using",
            _ => unreachable!(),
        };
        return format!(
            "{action} {} · {}…",
            execution.server_id, execution.tool_name
        );
    }
    if !message.tool_executions.is_empty() {
        "Waiting for model…".into()
    } else if message.thinking.is_empty() {
        "Contacting provider…".into()
    } else {
        "Thinking…".into()
    }
}

fn render_tool_executions(
    app: &OneChat,
    message: &AssistantResponse,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> Option<AnyElement> {
    if message.tool_executions.is_empty() {
        return None;
    }

    Some(
        div()
            .mb_4()
            .flex()
            .flex_col()
            .gap_2()
            .children(
                message
                    .tool_executions
                    .iter()
                    .map(|execution| render_tool_execution(app, execution, typography, cx)),
            )
            .into_any_element(),
    )
}

fn render_tool_execution(
    app: &OneChat,
    execution: &ToolExecution,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let expanded = app.tool_execution_expanded(&execution.id);
    let status = tool_status_text(execution);
    let danger = matches!(
        execution.status,
        ToolExecutionStatus::Failed | ToolExecutionStatus::Interrupted
    );
    let active = execution.status.is_active();
    let execution_id = execution.id.clone();
    let mut card = div()
        .id(SharedString::from(format!(
            "tool-execution-{}",
            execution.id
        )))
        .rounded_xl()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .px_3()
        .py_2()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(render_icon(
                    AppIcon::Plug,
                    if danger {
                        IconTone::Danger
                    } else if active {
                        IconTone::Accent
                    } else {
                        IconTone::Muted
                    },
                    16.0,
                    cx,
                ))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .text_size(px(typography.metadata_size))
                        .line_height(px(typography.metadata_line_height))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("{} · {}", execution.server_id, execution.tool_name)),
                )
                .child(
                    div()
                        .rounded_full()
                        .bg(if danger {
                            if cx.theme().is_dark() {
                                rgba(0xff453a24).into()
                            } else {
                                rgba(0xd7001518).into()
                            }
                        } else {
                            cx.theme().popover
                        })
                        .px_2()
                        .py_1()
                        .text_size(px(typography.micro_size))
                        .line_height(px(typography.micro_line_height))
                        .text_color(if danger {
                            cx.theme().danger
                        } else if active {
                            cx.theme().primary
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child(status),
                )
                .child(
                    icon_button(
                        SharedString::from(format!("toggle-tool-{}", execution.id)),
                        if expanded {
                            AppIcon::ChevronUp
                        } else {
                            AppIcon::ChevronDown
                        },
                        IconTone::Muted,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_tool_execution(execution_id.clone(), cx)
                    })),
                ),
        );

    if expanded {
        let arguments = serde_json::to_string_pretty(&execution.arguments)
            .unwrap_or_else(|_| execution.arguments.to_string());
        card = card.child(
            div()
                .pt_3()
                .border_t_1()
                .border_color(cx.theme().border)
                .flex()
                .flex_col()
                .gap_3()
                .child(tool_detail(
                    &execution.id,
                    "ARGUMENTS",
                    arguments,
                    false,
                    app,
                    typography,
                    cx,
                ))
                .children(execution.result.as_ref().map(|result| {
                    tool_detail(
                        &execution.id,
                        "RESULT",
                        result.clone(),
                        false,
                        app,
                        typography,
                        cx,
                    )
                }))
                .children(execution.error.as_ref().map(|error| {
                    tool_detail(
                        &execution.id,
                        "ERROR",
                        error.clone(),
                        true,
                        app,
                        typography,
                        cx,
                    )
                })),
        );
    }
    card.into_any_element()
}

fn tool_detail(
    execution_id: &str,
    label: &'static str,
    content: String,
    danger: bool,
    app: &OneChat,
    typography: MessageTypography,
    cx: &App,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(typography.micro_size))
                .line_height(px(typography.micro_line_height))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if danger {
                    cx.theme().danger
                } else {
                    cx.theme().muted_foreground
                })
                .child(label),
        )
        .child(
            div()
                .font(crate::desktop::ui::theme::code_font(cx))
                .text_size(px(typography.micro_size))
                .line_height(px(typography.micro_line_height + 4.0))
                .text_color(if danger {
                    cx.theme().danger
                } else {
                    cx.theme().foreground
                })
                .whitespace_normal()
                .child(SelectableText::new(
                    SharedString::from(format!("tool-{}-{}", label.to_lowercase(), execution_id)),
                    content,
                    app.chat.text_selection.clone(),
                    selection_color(cx.theme().is_dark()),
                )),
        )
        .into_any_element()
}

pub(super) fn tool_status_text(execution: &ToolExecution) -> String {
    let label = match execution.status {
        ToolExecutionStatus::Queued => "Queued",
        ToolExecutionStatus::Running => "Running",
        ToolExecutionStatus::Completed => "Completed",
        ToolExecutionStatus::Failed => "Failed",
        ToolExecutionStatus::Stopped => "Stopped",
        ToolExecutionStatus::Interrupted => "Interrupted",
    };
    execution.duration_ms.map_or_else(
        || label.to_string(),
        |duration| format!("{label} · {}", format_tool_duration(duration)),
    )
}

pub(super) fn format_tool_duration(duration_ms: u64) -> String {
    if duration_ms < 1_000 {
        format!("{duration_ms} ms")
    } else {
        format!("{}.{:01} s", duration_ms / 1_000, duration_ms % 1_000 / 100)
    }
}

fn render_reasoning(
    app: &OneChat,
    message: &AssistantResponse,
    request: Option<&RequestInfo>,
    typography: MessageTypography,
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
                .bg(cx.theme().popover)
                .px_2()
                .py_1()
                .flex()
                .items_center()
                .gap_1()
                .text_size(px(typography.micro_size))
                .line_height(px(typography.micro_line_height))
                .text_color(if live {
                    cx.theme().primary
                } else {
                    cx.theme().muted_foreground
                })
                .children(live.then(|| div().size(px(5.0)).rounded_full().bg(cx.theme().primary)))
                .child(duration),
        );
    }
    let thinking_id = message.id.clone();
    controls = controls.child(
        icon_button(
            SharedString::from(format!("thinking-{}", message.id)),
            if expanded {
                AppIcon::ChevronUp
            } else {
                AppIcon::ChevronDown
            },
            IconTone::Accent,
            cx,
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
            selection_color(cx.theme().is_dark()),
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
                    f32::from(scroll.max_offset().y),
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
            .bg(cx.theme().muted)
            .p_4()
            .text_size(px(typography.secondary_size))
            .line_height(px(typography.secondary_line_height))
            .text_color(cx.theme().muted_foreground)
            .child(
                div()
                    .pb_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(typography.micro_size))
                            .line_height(px(typography.micro_line_height))
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

fn status_badge(status: MessageStatus, typography: MessageTypography, cx: &App) -> AnyElement {
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
            if cx.theme().is_dark() {
                rgba(0xff453a24).into()
            } else {
                rgba(0xd7001518).into()
            }
        } else {
            cx.theme().muted
        })
        .px_2()
        .py_1()
        .text_size(px(typography.micro_size))
        .line_height(px(typography.micro_line_height))
        .text_color(if danger {
            cx.theme().danger
        } else {
            cx.theme().muted_foreground
        })
        .child(label)
        .into_any_element()
}

fn render_error_card(
    app: &OneChat,
    message: &AssistantResponse,
    request: Option<&RequestInfo>,
    latest: bool,
    generating: bool,
    typography: MessageTypography,
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
            .bg(if cx.theme().is_dark() {
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
                    .text_size(px(typography.metadata_size))
                    .line_height(px(typography.metadata_line_height))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().danger)
                    .child(summary),
            )
            .children(expanded.then(|| {
                div()
                    .text_size(px(typography.metadata_size))
                    .line_height(px(typography.metadata_line_height))
                    .text_color(cx.theme().muted_foreground)
                    .child(
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
                        primary_icon_button(
                            SharedString::from(format!("retry-message-{}", message.id)),
                            AppIcon::Regenerate,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.regenerate_assistant(retry_id.clone(), cx)
                        }))
                    }))
                    .children(detail.map(|_| {
                        large_icon_button(
                            SharedString::from(format!("error-detail-{}", message.id)),
                            if expanded {
                                AppIcon::ChevronUp
                            } else {
                                AppIcon::ChevronDown
                            },
                            IconTone::Muted,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_error_detail(detail_id.clone(), cx)
                        }))
                    })),
            )
            .into_any_element(),
    )
}
