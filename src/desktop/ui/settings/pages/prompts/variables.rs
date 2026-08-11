use super::super::super::*;

const BUILTIN_VARIABLES: [(&str, &str); 7] = [
    ("onechat.date", "Current local date"),
    ("onechat.datetime", "Current local date and time"),
    ("onechat.os", "Operating system"),
    ("onechat.conversation.id", "Conversation identifier"),
    ("onechat.conversation.title", "Conversation title"),
    ("onechat.model.name", "Selected model"),
    ("onechat.provider.name", "Selected provider"),
];

pub(super) fn prompt_variables_content(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .child(variable_privacy_notice(cx))
        .child(custom_variables_content(app, cx))
        .child(setting_divider(cx))
        .child(builtin_variables_content(app, cx))
        .into_any_element()
}

fn variable_privacy_notice(cx: &App) -> AnyElement {
    div()
        .min_w_0()
        .mx_2()
        .mt_2()
        .mb_1()
        .rounded_lg()
        .bg(cx.theme().accent.opacity(0.45))
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .gap_2()
        .child(render_icon(AppIcon::Info, IconTone::Accent, 15.0, cx))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .whitespace_normal()
                .text_size(px(12.0))
                .line_height(px(18.0))
                .text_color(cx.theme().muted_foreground)
                .child(
                    "Resolved values become part of the system prompt and are sent to the selected model provider.",
                ),
        )
        .into_any_element()
}

fn custom_variables_content(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    if app.settings().prompt_variables.is_empty() {
        return div()
            .px_4()
            .py_5()
            .flex()
            .flex_col()
            .items_center()
            .text_center()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("No custom variables"),
            )
            .child(
                div()
                    .pt_1()
                    .text_size(px(12.0))
                    .text_color(cx.theme().muted_foreground)
                    .child("Add text, an environment value, or command output."),
            )
            .into_any_element();
    }

    div()
        .w_full()
        .flex()
        .flex_col()
        .children(
            app.settings()
                .prompt_variables
                .iter()
                .map(|(name, source)| {
                    let edit_name = name.clone();
                    let delete_name = name.clone();
                    let placeholder = format!("{{{{{name}}}}}");
                    div()
                        .id(SharedString::from(format!("prompt-variable-{name}")))
                        .min_h(px(64.0))
                        .rounded_lg()
                        .px_3()
                        .py_2()
                        .flex()
                        .items_center()
                        .gap_3()
                        .hover(|style| style.bg(cx.theme().list_hover))
                        .child(
                            div()
                                .size(px(32.0))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(9.0))
                                .bg(cx.theme().muted)
                                .child(render_icon(
                                    variable_icon(source),
                                    IconTone::Muted,
                                    16.0,
                                    cx,
                                )),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .child(
                                    div()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(placeholder.clone()),
                                )
                                .child(
                                    div()
                                        .pt_1()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!(
                                            "{} · {}",
                                            source.label(),
                                            source.preview()
                                        )),
                                ),
                        )
                        .child(CopyButton::new(
                            SharedString::from(format!("copy-prompt-variable-{name}")),
                            placeholder,
                        ))
                        .child(
                            Compact
                                .icon_action(
                                    SharedString::from(format!("edit-prompt-variable-{name}")),
                                    AppIcon::Pencil,
                                    IconTone::Muted,
                                    "Edit variable",
                                    cx,
                                )
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.begin_edit_prompt_variable(edit_name.clone(), window, cx)
                                })),
                        )
                        .child(
                            Compact
                                .icon_action(
                                    SharedString::from(format!("delete-prompt-variable-{name}")),
                                    AppIcon::Trash,
                                    IconTone::Danger,
                                    "Delete variable",
                                    cx,
                                )
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.request_delete_prompt_variable(
                                        delete_name.clone(),
                                        window,
                                        cx,
                                    )
                                })),
                        )
                }),
        )
        .into_any_element()
}

fn variable_icon(source: &PromptVariableSource) -> AppIcon {
    match source {
        PromptVariableSource::Text { .. } => AppIcon::FileText,
        PromptVariableSource::Environment { .. } => AppIcon::Key,
        PromptVariableSource::Command { .. } => AppIcon::Command,
    }
}

fn builtin_variables_content(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let expanded = app.settings_ui.prompt_builtins_expanded;
    div()
        .w_full()
        .flex()
        .flex_col()
        .child(
            div()
                .w_full()
                .h(px(48.0))
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(render_icon(AppIcon::Braces, IconTone::Muted, 15.0, cx))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Built-in Variables"),
                        )
                        .child(status_pill(
                            BUILTIN_VARIABLES.len().to_string(),
                            false,
                            StatusPillBackground::Muted,
                            cx,
                        )),
                )
                .child(
                    Compact
                        .icon_action(
                            "toggle-built-in-prompt-variables",
                            if expanded {
                                AppIcon::ChevronUp
                            } else {
                                AppIcon::ChevronDown
                            },
                            IconTone::Muted,
                            if expanded {
                                "Collapse built-in variables"
                            } else {
                                "Expand built-in variables"
                            },
                            cx,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_prompt_builtins(cx))),
                ),
        )
        .when(expanded, |content| {
            content.child(
                div()
                    .mx_2()
                    .mb_2()
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().border)
                    .children(BUILTIN_VARIABLES.iter().enumerate().map(
                        |(index, (name, description))| {
                            let placeholder = format!("{{{{{name}}}}}");
                            div()
                                .min_h(px(48.0))
                                .px_3()
                                .flex()
                                .items_center()
                                .gap_3()
                                .when(index + 1 < BUILTIN_VARIABLES.len(), |row| {
                                    row.border_b_1().border_color(cx.theme().border)
                                })
                                .child(
                                    div()
                                        .w(px(245.0))
                                        .flex_none()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(placeholder.clone()),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(*description),
                                )
                                .child(CopyButton::new(
                                    SharedString::from(format!("copy-builtin-{name}")),
                                    placeholder,
                                ))
                        },
                    )),
            )
        })
        .into_any_element()
}
