use super::super::super::*;

pub(super) fn translation_prompts_content(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let settings = app.settings();
    let customized = settings.translation_system_prompt.trim() != DEFAULT_TRANSLATION_SYSTEM_PROMPT
        || settings.translation_user_prompt.trim() != DEFAULT_TRANSLATION_USER_PROMPT;
    let status = div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Translation Instructions"),
        )
        .child(status_pill(
            if customized { "Customized" } else { "Built-in" },
            customized,
            StatusPillBackground::Muted,
            cx,
        ));

    if let (Some(system_editor), Some(user_editor)) = (
        &app.settings_ui.translation_system_prompt_editor,
        &app.settings_ui.translation_user_prompt_editor,
    ) {
        let mut leading_action = div().into_any_element();
        if customized {
            leading_action = Compact
                .icon_action(
                    "reset-translation-prompt-editors",
                    AppIcon::Regenerate,
                    IconTone::Muted,
                    "Use built-in translation prompts",
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.reset_translation_prompt_defaults(cx)))
                .into_any_element();
        }
        return stretching_column()
            .w_full()
            .p_3()
            .gap_4()
            .child(status)
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(cx.theme().muted_foreground)
                    .child("Available variables: {{text}}, {{sourceLanguage}}, {{targetLanguage}}. At least one prompt must include {{text}}."),
            )
            .child(
                Form::vertical()
                    .child(
                        Field::new().label("System Prompt").child(
                            Textarea::new(system_editor)
                                .aria_label("Default translation system prompt")
                                .h(px(180.0)),
                        ),
                    )
                    .child(
                        Field::new().label("User Prompt").child(
                            Textarea::new(user_editor)
                                .aria_label("Default translation user prompt")
                                .h(px(180.0)),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(leading_action)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Compact
                                    .icon_action(
                                        "cancel-translation-prompt-defaults",
                                        AppIcon::Close,
                                        IconTone::Muted,
                                        "Cancel",
                                        cx,
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.cancel_translation_prompt_edit(cx)
                                    })),
                            )
                            .child(
                                Compact
                                    .primary_icon_action(
                                        "save-translation-prompt-defaults",
                                        AppIcon::Save,
                                        "Save",
                                        cx,
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.save_translation_prompt_defaults(cx)
                                    })),
                            ),
                    ),
            )
            .into_any_element();
    }

    let mut actions = div().flex_none().flex().items_center().gap_2();
    if customized {
        actions = actions.child(
            Compact
                .icon_action(
                    "reset-translation-prompt-defaults",
                    AppIcon::Regenerate,
                    IconTone::Muted,
                    "Use built-in translation prompts",
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.reset_translation_prompt_defaults(cx))),
        );
    }
    actions = actions.child(
        Compact
            .icon_action(
                "edit-translation-prompt-defaults",
                AppIcon::Pencil,
                IconTone::Muted,
                "Edit translation prompts",
                cx,
            )
            .on_click(
                cx.listener(|this, _, window, cx| this.begin_edit_translation_prompts(window, cx)),
            ),
    );

    stretching_column()
        .w_full()
        .p_3()
        .gap_3()
        .child(status)
        .child(prompt_preview(
            "System Prompt",
            &settings.translation_system_prompt,
            cx,
        ))
        .child(prompt_preview(
            "User Prompt",
            &settings.translation_user_prompt,
            cx,
        ))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .min_w_0()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child("Variables: {{text}}, {{sourceLanguage}}, {{targetLanguage}}. {{text}} is required."),
                )
                .child(actions),
        )
        .into_any_element()
}

fn prompt_preview(label: &'static str, prompt: &str, cx: &App) -> AnyElement {
    div()
        .rounded_lg()
        .bg(cx.theme().muted)
        .px_3()
        .py_3()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground)
                .child(label),
        )
        .child(
            div()
                .max_h(px(54.0))
                .overflow_hidden()
                .text_size(px(12.0))
                .line_height(px(18.0))
                .text_color(cx.theme().muted_foreground)
                .child(text_summary(prompt, 420, None)),
        )
        .into_any_element()
}
