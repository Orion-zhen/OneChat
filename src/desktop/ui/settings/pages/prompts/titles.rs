use super::super::super::*;

pub(super) fn title_prompt_content(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let prompt = app.settings().title_generation_system_prompt.trim();
    let customized = prompt != DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT;
    let status = div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Title Instructions"),
        )
        .child(status_pill(
            if customized { "Customized" } else { "Built-in" },
            customized,
            StatusPillBackground::Muted,
            cx,
        ));

    if let Some(editor) = &app.settings_ui.title_prompt_editor {
        let mut actions = div().flex().items_center().justify_between().gap_2();
        if customized {
            actions = actions.child(
                Compact
                    .icon_action(
                        "reset-title-generation-prompt-editor",
                        AppIcon::Regenerate,
                        IconTone::Muted,
                        "Use default title prompt",
                        cx,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.reset_title_generation_prompt(cx))),
            );
        } else {
            actions = actions.child(div());
        }
        return stretching_column()
            .w_full()
            .p_3()
            .gap_4()
            .child(status)
            .child(
                Form::vertical().child(
                    Field::new().label("Prompt").required(true).child(
                        Input::new(editor)
                            .aria_label("Title generation prompt")
                            .h(px(220.0)),
                    ),
                ),
            )
            .child(
                actions.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Compact
                                .icon_action(
                                    "cancel-title-generation-system-prompt",
                                    AppIcon::Close,
                                    IconTone::Muted,
                                    "Cancel",
                                    cx,
                                )
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.cancel_title_prompt_edit(cx)),
                                ),
                        )
                        .child(
                            Compact
                                .primary_icon_action(
                                    "save-title-generation-system-prompt",
                                    AppIcon::Save,
                                    "Save",
                                    cx,
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.save_title_prompt(cx))),
                        ),
                ),
            )
            .into_any_element();
    }

    let preview = if prompt.is_empty() {
        "No title instructions.".to_string()
    } else {
        text_summary(prompt, 420, None)
    };
    let mut actions = div().flex_none().flex().items_center().gap_2();
    if customized {
        actions = actions.child(
            Compact
                .icon_action(
                    "reset-title-generation-prompt",
                    AppIcon::Regenerate,
                    IconTone::Muted,
                    "Use default title prompt",
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.reset_title_generation_prompt(cx))),
        );
    }
    actions = actions.child(
        Compact
            .icon_action(
                "edit-title-generation-system-prompt",
                AppIcon::Pencil,
                IconTone::Muted,
                "Edit title prompt",
                cx,
            )
            .on_click(cx.listener(|this, _, window, cx| this.begin_edit_title_prompt(window, cx))),
    );

    stretching_column()
        .w_full()
        .p_3()
        .gap_3()
        .child(status)
        .child(
            div()
                .rounded_lg()
                .bg(cx.theme().muted)
                .px_3()
                .py_3()
                .flex()
                .items_start()
                .justify_between()
                .gap_5()
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .max_h(px(54.0))
                        .overflow_hidden()
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(preview),
                )
                .child(actions),
        )
        .into_any_element()
}
