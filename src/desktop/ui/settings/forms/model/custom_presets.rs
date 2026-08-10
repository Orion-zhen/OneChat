use super::*;

pub(super) fn custom_reasoning_presets(
    editor: &ModelReasoningEditor,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let provider_default = editor.custom_default.is_none();
    let mut content = div()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Presets"),
                )
                .child(
                    primary_icon_action(
                        "add-custom-reasoning-preset",
                        AppIcon::Plus,
                        "Add reasoning preset",
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.add_custom_reasoning_preset(window, cx)
                    })),
                ),
        )
        .child(
            div()
                .rounded(px(10.0))
                .bg(cx.theme().muted)
                .px_3()
                .py_2()
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_sm().child("Provider Default"))
                .child(
                    default_reasoning_action(
                        "custom-reasoning-default-provider",
                        provider_default,
                        cx,
                    )
                    .on_click(
                        cx.listener(|this, _, _, cx| this.set_custom_reasoning_default(None, cx)),
                    ),
                ),
        );
    for (index, preset) in editor.custom_presets.iter().enumerate() {
        let selected = editor.custom_default == Some(index);
        let can_move_up = index > 0;
        let can_move_down = index + 1 < editor.custom_presets.len();
        let card = div()
            .rounded(px(12.0))
            .border_1()
            .border_color(cx.theme().border)
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap_2()
                    .child(reasoning_identity_field("ID", &preset.id, cx))
                    .child(reasoning_identity_field(
                        "Name (optional)",
                        &preset.name,
                        cx,
                    ))
                    .child(
                        div()
                            .h(px(40.0))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                icon_action(
                                    SharedString::from(format!("move-reasoning-preset-up-{index}")),
                                    AppIcon::ArrowUp,
                                    IconTone::Muted,
                                    "Move preset up",
                                    cx,
                                )
                                .disabled(!can_move_up)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.move_custom_reasoning_preset(index, -1, cx)
                                    },
                                )),
                            )
                            .child(
                                icon_action(
                                    SharedString::from(format!(
                                        "move-reasoning-preset-down-{index}"
                                    )),
                                    AppIcon::ArrowDown,
                                    IconTone::Muted,
                                    "Move preset down",
                                    cx,
                                )
                                .disabled(!can_move_down)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.move_custom_reasoning_preset(index, 1, cx)
                                    },
                                )),
                            )
                            .child(
                                default_reasoning_action(
                                    SharedString::from(format!("custom-reasoning-default-{index}")),
                                    selected,
                                    cx,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.set_custom_reasoning_default(Some(index), cx)
                                    },
                                )),
                            )
                            .child(
                                danger_icon_action(
                                    SharedString::from(format!("remove-reasoning-preset-{index}")),
                                    AppIcon::Trash,
                                    "Remove reasoning preset",
                                    cx,
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.remove_custom_reasoning_preset(index, cx)
                                    },
                                )),
                            ),
                    ),
            )
            .child(reasoning_parameter_list(
                index,
                ReasoningParameterScope::Request,
                "Request Parameters",
                &preset.request_parameters,
                cx,
            ))
            .child(reasoning_parameter_list(
                index,
                ReasoningParameterScope::ChatTemplateKwargs,
                "chat_template_kwargs",
                &preset.chat_template_kwargs,
                cx,
            ));
        content = content.child(card);
    }
    content.into_any_element()
}
