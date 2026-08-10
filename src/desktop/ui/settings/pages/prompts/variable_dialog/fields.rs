use super::*;

pub(super) fn variable_kind_selector(
    editor: &PromptVariableEditor,
    app_entity: &Entity<OneChat>,
    cx: &App,
) -> AnyElement {
    let kinds = div()
        .w_full()
        .flex()
        .items_center()
        .gap_1()
        .rounded(px(10.0))
        .bg(cx.theme().muted)
        .p_1()
        .children(
            [
                (
                    "prompt-variable-kind-text",
                    PromptVariableKind::Text,
                    AppIcon::FileText,
                    "Text source",
                ),
                (
                    "prompt-variable-kind-environment",
                    PromptVariableKind::Environment,
                    AppIcon::Key,
                    "Environment source",
                ),
                (
                    "prompt-variable-kind-command",
                    PromptVariableKind::Command,
                    AppIcon::Command,
                    "Command source",
                ),
            ]
            .into_iter()
            .map(|(id, kind, icon, tooltip)| {
                let selected = editor.kind == kind;
                let kind_app = app_entity.clone();
                Button::new(id)
                    .ghost()
                    .flex_1()
                    .h(px(32.0))
                    .rounded(px(7.0))
                    .tooltip(tooltip)
                    .selected(selected)
                    .toggled(selected)
                    .when(selected, |button| button.bg(cx.theme().popover))
                    .child(render_icon(
                        icon,
                        if selected {
                            IconTone::Accent
                        } else {
                            IconTone::Muted
                        },
                        16.0,
                        cx,
                    ))
                    .on_click(move |_, _, cx| {
                        kind_app.update(cx, |app, cx| app.set_prompt_variable_kind(kind, cx));
                    })
            }),
        );

    kinds.into_any_element()
}

pub(super) fn variable_name_field(editor: &PromptVariableEditor, cx: &App) -> AnyElement {
    let name_field = if let Some(name) = editor.original_name() {
        readonly_field("Name", format!("{{{{{name}}}}}"), cx)
    } else {
        Field::new()
            .label("Name")
            .required(true)
            .child(
                Input::new(&editor.name)
                    .aria_label("Variable name")
                    .large()
                    .rounded(px(12.0)),
            )
            .into_any_element()
    };

    name_field.into_any_element()
}

pub(super) fn variable_value_field(editor: &PromptVariableEditor) -> AnyElement {
    let value_field = match editor.kind {
        PromptVariableKind::Text => Field::new()
            .label("Text")
            .child(
                Input::new(&editor.text)
                    .aria_label("Text")
                    .large()
                    .rounded(px(12.0))
                    .h(px(150.0)),
            )
            .into_any_element(),
        PromptVariableKind::Environment => Field::new()
            .label("Environment Variable")
            .required(true)
            .child(
                Input::new(&editor.environment)
                    .aria_label("Environment variable")
                    .large()
                    .rounded(px(12.0)),
            )
            .into_any_element(),
        PromptVariableKind::Command => Field::new()
            .label("Shell Script")
            .required(true)
            .child(
                Input::new(&editor.script)
                    .aria_label("Shell script")
                    .large()
                    .rounded(px(12.0))
                    .h(px(150.0)),
            )
            .into_any_element(),
    };

    value_field.into_any_element()
}
