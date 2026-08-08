use super::*;

pub(super) fn render_system_prompt_card(
    app: &OneChat,
    message_max_width: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let conversation = app
        .current_conversation()
        .expect("system prompt card requires a conversation");
    let source = app.system_prompt_label(&conversation.system_prompt);
    let actions = match app.chat.system_prompt_mode {
        SystemPromptMode::Compact => {
            div()
                .flex()
                .gap_1()
                .child(
                    large_icon_button(
                        "expand-system-prompt",
                        AppIcon::ChevronDown,
                        IconTone::Muted,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.expand_system_prompt(cx))),
                )
                .child(
                    large_icon_button("copy-system-prompt", AppIcon::Copy, IconTone::Muted, cx)
                        .on_click(cx.listener(|this, _, _, cx| this.copy_system_prompt(cx))),
                )
                .child(
                    large_icon_button("edit-system-prompt", AppIcon::Pencil, IconTone::Accent, cx)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.begin_edit_system_prompt(window, cx)
                        })),
                )
                .into_any_element()
        }
        SystemPromptMode::Expanded => div()
            .flex()
            .gap_1()
            .child(
                large_icon_button(
                    "collapse-system-prompt",
                    AppIcon::ChevronUp,
                    IconTone::Muted,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.collapse_system_prompt(cx))),
            )
            .child(
                large_icon_button(
                    "copy-system-prompt-expanded",
                    AppIcon::Copy,
                    IconTone::Muted,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.copy_system_prompt(cx))),
            )
            .child(
                large_icon_button(
                    "edit-system-prompt-expanded",
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
                primary_icon_button("save-system-prompt", AppIcon::Save, cx)
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
                large_icon_button("cancel-system-prompt", AppIcon::Close, IconTone::Muted, cx)
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
            .text_sm()
            .line_height(px(21.0))
            .text_color(cx.theme().muted_foreground)
            .child(prompt_preview(&conversation.system_prompt))
            .into_any_element(),
        SystemPromptMode::Expanded => div()
            .text_sm()
            .line_height(px(22.0))
            .whitespace_normal()
            .child(conversation.system_prompt.clone())
            .into_any_element(),
        SystemPromptMode::Editing => app
            .chat
            .system_prompt_editor
            .as_ref()
            .map(|editor| {
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_move(|_, _, cx| cx.stop_propagation())
                    .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_up_out(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_action(cx.listener(|this, _: &InputEscape, _, cx| {
                        this.cancel_system_prompt_edit(cx)
                    }))
                    .child(
                        Input::new(editor)
                            .aria_label("System prompt")
                            .bg(cx.theme().muted)
                            .text_size(px(15.0))
                            .line_height(px(22.0)),
                    )
                    .into_any_element()
            })
            .unwrap_or_else(|| {
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Opening editor…")
                    .into_any_element()
            }),
    };

    let card = div()
        .mx_auto()
        .mb_7()
        .w_full()
        .max_w(px(message_max_width))
        .rounded(px(16.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .shadow_xs()
        .px_4()
        .py_3()
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
                                .bg(cx.theme().accent)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(11.0))
                                .text_color(cx.theme().primary)
                                .child(render_icon(AppIcon::Command, IconTone::Accent, 13.0, cx)),
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
                                .bg(cx.theme().accent)
                                .px_2()
                                .py_1()
                                .text_size(px(10.0))
                                .text_color(cx.theme().primary)
                                .child(source),
                        ),
                )
                .child(actions),
        )
        .child(content);
    let animation_id = match app.chat.system_prompt_mode {
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

pub(super) fn prompt_preview(prompt: &str) -> String {
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
