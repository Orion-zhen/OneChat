use super::super::super::*;
use super::components::{field_label, readonly_field};

mod fields;

use fields::{variable_kind_selector, variable_name_field, variable_value_field};

pub(in crate::desktop::ui::settings) fn prompt_variable_dialog_body(
    app: &OneChat,
    app_entity: Entity<OneChat>,
    cx: &App,
) -> AnyElement {
    let editor = app
        .settings_ui
        .prompt_variable_editor
        .as_ref()
        .expect("prompt variable dialog requires an editor");
    let kinds = variable_kind_selector(editor, &app_entity, cx);
    let name_field = variable_name_field(editor, cx);
    let value_field = variable_value_field(editor);
    let mut content = stretching_column()
        .id("prompt-variable-dialog-body")
        .max_h(px(680.0))
        .overflow_y_scroll()
        .px_5()
        .pb_5()
        .gap_4()
        .child(name_field)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(field_label("Source", cx))
                .child(kinds),
        )
        .child(value_field);

    if editor.kind != PromptVariableKind::Text {
        content = content.child(variable_source_notice(editor.kind, cx));
    }

    if editor.kind == PromptVariableKind::Command {
        let advanced_app = app_entity.clone();
        let test_app = app_entity.clone();
        let running = matches!(editor.test_status, Some(PromptVariableTestStatus::Running));
        content = content
            .child(
                div()
                    .w_full()
                    .h(px(36.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Advanced Options"),
                    )
                    .child(
                        Compact
                            .icon_action(
                                "toggle-prompt-variable-advanced",
                                if editor.advanced_expanded {
                                    AppIcon::ChevronUp
                                } else {
                                    AppIcon::ChevronDown
                                },
                                IconTone::Muted,
                                if editor.advanced_expanded {
                                    "Collapse advanced options"
                                } else {
                                    "Expand advanced options"
                                },
                                cx,
                            )
                            .on_click(move |_, _, cx| {
                                advanced_app
                                    .update(cx, |app, cx| app.toggle_prompt_variable_advanced(cx));
                            }),
                    ),
            )
            .when(editor.advanced_expanded, |content| {
                content.child(
                    div()
                        .rounded_lg()
                        .bg(cx.theme().muted.opacity(0.55))
                        .p_3()
                        .child(
                            Form::vertical()
                                .child(
                                    Field::new().label("Working Directory").child(
                                        Input::new(&editor.cwd)
                                            .aria_label("Working directory")
                                            .large()
                                            .rounded(px(12.0)),
                                    ),
                                )
                                .child(
                                    Field::new()
                                        .label("Timeout (seconds)")
                                        .required(true)
                                        .child(
                                            Input::new(&editor.timeout_seconds)
                                                .aria_label("Timeout in seconds")
                                                .large()
                                                .rounded(px(12.0)),
                                        ),
                                ),
                        ),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(cx.theme().muted_foreground)
                            .child("Run once to inspect the output before saving."),
                    )
                    .child(
                        Compact
                            .icon_action(
                                "test-prompt-variable-command",
                                AppIcon::Command,
                                IconTone::Muted,
                                if running {
                                    "Command is running"
                                } else {
                                    "Test command"
                                },
                                cx,
                            )
                            .disabled(running)
                            .on_click(move |_, _, cx| {
                                test_app.update(cx, |app, cx| app.test_prompt_variable_command(cx));
                            }),
                    ),
            )
            .children(command_test_result(editor, cx));
    }

    content
        .children(app.settings_ui.form_error.as_deref().map(error_banner))
        .into_any_element()
}

fn variable_source_notice(kind: PromptVariableKind, cx: &App) -> AnyElement {
    let text = match kind {
        PromptVariableKind::Environment => {
            "The environment value is read only when referenced, then inserted into the prompt and sent to the model provider."
        }
        PromptVariableKind::Command => {
            "This script runs locally only when referenced. Its output is inserted into the prompt and sent to the model provider."
        }
        PromptVariableKind::Text => unreachable!(),
    };
    div()
        .w_full()
        .min_w_0()
        .rounded_lg()
        .bg(cx.theme().warning.opacity(0.12))
        .p_3()
        .flex()
        .items_center()
        .gap_2()
        .child(render_icon(AppIcon::Info, IconTone::Muted, 15.0, cx))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .whitespace_normal()
                .text_size(px(12.0))
                .line_height(px(18.0))
                .child(text),
        )
        .into_any_element()
}

fn command_test_result(editor: &PromptVariableEditor, cx: &App) -> Option<AnyElement> {
    match editor.test_status.as_ref()? {
        PromptVariableTestStatus::Running => Some(
            div()
                .rounded_lg()
                .bg(cx.theme().muted)
                .p_3()
                .flex()
                .items_center()
                .gap_2()
                .child(Spinner::new().small().color(cx.theme().primary))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child("Running command…"),
                )
                .into_any_element(),
        ),
        PromptVariableTestStatus::Failed(error) => Some(
            Alert::error("prompt-variable-test-error", error.clone())
                .small()
                .into_any_element(),
        ),
        PromptVariableTestStatus::Succeeded {
            output,
            duration_ms,
        } => Some(
            div()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .overflow_hidden()
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .bg(cx.theme().muted)
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Command succeeded"),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{duration_ms} ms")),
                        ),
                )
                .child(
                    div()
                        .id("prompt-variable-test-output")
                        .max_h(px(160.0))
                        .overflow_y_scroll()
                        .p_3()
                        .font(crate::desktop::ui::theme::code_font(cx))
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .child(if output.is_empty() {
                            "No output".to_string()
                        } else {
                            output.clone()
                        }),
                )
                .into_any_element(),
        ),
    }
}
