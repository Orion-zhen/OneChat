use super::*;

const COLLAPSED_SYSTEM_PROMPT_HEIGHT: f32 = 88.0;

pub(super) fn render_prompt_setup_card(
    app: &OneChat,
    message_max_width: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let conversation = app
        .current_conversation()
        .expect("prompt setup card requires a conversation");
    let source = app.prompt_setup_label(conversation);
    let has_system_prompt = !conversation.system_prompt.trim().is_empty();
    let actions = match app.chat.system_prompt_mode {
        SystemPromptMode::Compact => {
            div()
                .flex()
                .gap_1()
                .children(has_system_prompt.then(|| {
                    large_icon_button(
                        "expand-prompt-setup",
                        AppIcon::ChevronDown,
                        IconTone::Muted,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.expand_system_prompt(cx)))
                }))
                .child(
                    large_icon_button("edit-prompt-setup", AppIcon::Pencil, IconTone::Accent, cx)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.begin_edit_system_prompt(window, cx)
                        })),
                )
                .into_any_element()
        }
        SystemPromptMode::Expanded => div()
            .flex()
            .gap_1()
            .children(has_system_prompt.then(|| {
                large_icon_button(
                    "collapse-prompt-setup",
                    AppIcon::ChevronUp,
                    IconTone::Muted,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.collapse_system_prompt(cx)))
            }))
            .child(
                large_icon_button(
                    "edit-prompt-setup-expanded",
                    AppIcon::Pencil,
                    IconTone::Accent,
                    cx,
                )
                .on_click(
                    cx.listener(|this, _, window, cx| this.begin_edit_system_prompt(window, cx)),
                ),
            )
            .into_any_element(),
        SystemPromptMode::Editing => div()
            .flex()
            .gap_2()
            .child(
                primary_icon_button("save-prompt-setup", AppIcon::Save, cx)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.save_system_prompt(cx);
                            cx.stop_propagation();
                        }),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.save_system_prompt(cx))),
            )
            .child(
                large_icon_button("cancel-prompt-setup", AppIcon::Close, IconTone::Muted, cx)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.cancel_system_prompt_edit(cx);
                            cx.stop_propagation();
                        }),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_system_prompt_edit(cx))),
            )
            .into_any_element(),
    };

    let content = match app.chat.system_prompt_mode {
        SystemPromptMode::Compact => div()
            .flex()
            .flex_col()
            .children(has_system_prompt.then(|| {
                prompt_text_section(
                    "System Prompt",
                    conversation.system_prompt.clone(),
                    typography,
                    true,
                    cx,
                )
            }))
            .into_any_element(),
        SystemPromptMode::Expanded => div()
            .flex()
            .flex_col()
            .children(has_system_prompt.then(|| {
                prompt_text_section(
                    "System Prompt",
                    conversation.system_prompt.clone(),
                    typography,
                    false,
                    cx,
                )
            }))
            .into_any_element(),
        SystemPromptMode::Editing => {
            let mut fields = div()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_move(|_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up_out(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_action(
                    cx.listener(|this, _: &InputEscape, _, cx| this.cancel_system_prompt_edit(cx)),
                )
                .flex()
                .flex_col()
                .gap_4();
            if let Some(editor) = &app.chat.system_prompt_editor {
                fields = fields.child(prompt_editor(
                    "System Prompt",
                    None,
                    "System prompt",
                    editor,
                    typography,
                    cx,
                ));
            }
            if let Some(editor) = &app.chat.assistant_opening_editor {
                fields = fields.child(prompt_editor(
                    "Assistant Opening",
                    Some("Optional"),
                    "Assistant opening",
                    editor,
                    typography,
                    cx,
                ));
            }
            fields.into_any_element()
        }
    };

    let editing = app.chat.system_prompt_mode == SystemPromptMode::Editing;
    let show_content = has_system_prompt || editing;
    let content = if editing {
        content
    } else {
        div()
            .rounded(px(16.0))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .shadow_xs()
            .p_4()
            .child(content)
            .into_any_element()
    };
    let section = div()
        .mx_auto()
        .mb_6()
        .w_full()
        .max_w(px(message_max_width))
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .px_1()
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
                                .bg(cx.theme().accent)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(cx.theme().primary)
                                .child(render_icon(AppIcon::Command, IconTone::Accent, 13.0, cx)),
                        )
                        .child(
                            div()
                                .text_size(px(typography.metadata_size))
                                .line_height(px(typography.metadata_line_height))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Prompt Setup"),
                        )
                        .child(
                            div()
                                .rounded_full()
                                .bg(cx.theme().accent)
                                .px_2()
                                .py_1()
                                .text_size(px(typography.micro_size))
                                .line_height(px(typography.micro_line_height))
                                .text_color(cx.theme().primary)
                                .child(source),
                        ),
                )
                .child(actions),
        )
        .children(show_content.then_some(content));
    let animation_id = match app.chat.system_prompt_mode {
        SystemPromptMode::Compact => "prompt-setup-compact",
        SystemPromptMode::Expanded => "prompt-setup-expanded",
        SystemPromptMode::Editing => "prompt-setup-editing",
    };
    section
        .with_animation(
            animation_id,
            Animation::new(Duration::from_millis(200)).with_easing(ease_out_quint()),
            |section, delta| {
                section
                    .opacity(0.78 + delta * 0.22)
                    .mt(px(8.0 * (1.0 - delta)))
            },
        )
        .into_any_element()
}

fn prompt_text_section(
    label: &'static str,
    content: String,
    typography: MessageTypography,
    muted: bool,
    cx: &App,
) -> AnyElement {
    let text = div()
        .text_size(px(typography.secondary_size))
        .line_height(px(typography.secondary_line_height))
        .whitespace_normal()
        .when(muted, |text| {
            text.max_h(px(COLLAPSED_SYSTEM_PROMPT_HEIGHT))
                .overflow_hidden()
        })
        .child(content);
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(editor_label(label, None, typography, cx))
        .child(if muted {
            text.text_color(cx.theme().muted_foreground)
                .into_any_element()
        } else {
            text.into_any_element()
        })
        .into_any_element()
}

fn prompt_editor(
    label: &'static str,
    suffix: Option<&'static str>,
    aria_label: &'static str,
    editor: &gpui::Entity<gpui_component::input::TextareaState>,
    typography: MessageTypography,
    cx: &App,
) -> AnyElement {
    div()
        .rounded(px(20.0))
        .border_1()
        .border_color(crate::desktop::ui::theme::palette(cx).accent_border)
        .bg(cx.theme().popover)
        .shadow_xs()
        .p_3()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_color(cx.theme().muted_foreground)
                .child(render_icon(AppIcon::Pencil, IconTone::Accent, 14.0, cx))
                .child(editor_label(label, suffix, typography, cx)),
        )
        .child(
            Textarea::new(editor)
                .aria_label(aria_label)
                .appearance(false)
                .w_full()
                .text_size(px(typography.body_size))
                .line_height(px(typography.body_line_height)),
        )
        .into_any_element()
}

fn editor_label(
    label: &'static str,
    suffix: Option<&'static str>,
    typography: MessageTypography,
    cx: &App,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .text_size(px(typography.micro_size))
        .line_height(px(typography.micro_line_height))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(label)
        .children(suffix.map(|suffix| div().font_weight(FontWeight::NORMAL).child(suffix)))
        .into_any_element()
}
