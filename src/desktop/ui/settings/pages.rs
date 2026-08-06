use super::*;

pub(super) fn general_page(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let theme = app.settings().theme.label();
    let appearance = div().flex().flex_col().gap_2().child(setting_row(
        "Theme",
        "Match the Mac or choose a fixed appearance.",
        button("cycle-theme", theme, colors)
            .on_click(cx.listener(|this, _, _, cx| this.cycle_theme(cx))),
        colors,
    ));

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "General",
                "Choose how OneChat looks and responds.",
                colors,
            ))
            .child(section("Appearance", None, appearance, colors)),
    )
}

pub(super) fn system_prompts_page(
    app: &OneChat,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let content = if let Some(editor) = &app.settings_ui.default_system_prompt_editor {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(editor.clone())
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        button("cancel-default-system-prompt", "Cancel", colors).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.cancel_default_system_prompt_edit(cx)
                            }),
                        ),
                    )
                    .child(
                        primary_button("save-default-system-prompt", "Save", colors).on_click(
                            cx.listener(|this, _, _, cx| this.save_default_system_prompt(cx)),
                        ),
                    ),
            )
            .into_any_element()
    } else {
        let prompt = app.data.snapshot.settings.default_system_prompt.trim();
        div()
            .flex()
            .items_start()
            .justify_between()
            .gap_5()
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Default Prompt"),
                    )
                    .child(
                        div()
                            .pt_1()
                            .text_size(px(12.0))
                            .line_height(px(18.0))
                            .text_color(colors.muted)
                            .child(if prompt.is_empty() {
                                "New conversations start without a System Prompt.".into()
                            } else {
                                prompt_preview(prompt)
                            }),
                    ),
            )
            .child(
                button("edit-default-system-prompt", "Edit", colors).on_click(
                    cx.listener(|this, _, _, cx| this.begin_edit_default_system_prompt(cx)),
                ),
            )
            .into_any_element()
    };

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "System Prompts",
                "Set the instructions copied into every new conversation.",
                colors,
            ))
            .child(section(
                "Default",
                Some("Existing conversations keep their own prompt."),
                content,
                colors,
            )),
    )
}
