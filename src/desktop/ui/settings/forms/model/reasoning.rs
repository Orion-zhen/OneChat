use super::*;

pub(super) fn model_reasoning_form(
    editor: &ModelReasoningEditor,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let enabled = editor.enabled;
    let header = div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Reasoning"),
                )
                .child(
                    div()
                        .pt_1()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child("Configure named reasoning presets for this model."),
                ),
        )
        .child(
            Switch::new("model-reasoning-enabled")
                .small()
                .checked(enabled)
                .color(cx.theme().primary)
                .on_click(cx.listener(|this, value: &bool, _, cx| {
                    this.set_model_reasoning_enabled(*value, cx)
                })),
        );
    let mut content = div().w_full().flex().flex_col().gap_4().child(header);
    if !enabled {
        return content.into_any_element();
    }

    let mode = editor.mode;
    let mode_selector = div()
        .w_full()
        .flex()
        .items_center()
        .gap_1()
        .rounded(px(10.0))
        .bg(cx.theme().muted)
        .p_1()
        .children(
            [
                (ReasoningEditorMode::KnownApi, "Known API Format"),
                (ReasoningEditorMode::Custom, "Custom Parameters"),
            ]
            .into_iter()
            .map(|(candidate, label)| {
                let selected = mode == candidate;
                Button::new(SharedString::from(format!(
                    "reasoning-mode-{}",
                    candidate.index()
                )))
                .ghost()
                .flex_1()
                .h(px(32.0))
                .rounded(px(7.0))
                .label(label)
                .selected(selected)
                .toggled(selected)
                .when(selected, |button| button.bg(cx.theme().popover))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_model_reasoning_mode(candidate, window, cx)
                }))
            }),
        );
    content = content.child(mode_selector);
    match mode {
        ReasoningEditorMode::KnownApi => content
            .child(
                Field::new()
                    .label("API Format")
                    .description("Controls how the selected preset is encoded in the request")
                    .child(
                        Select::new(&editor.format_select)
                            .large()
                            .h(px(40.0))
                            .px(px(12.0))
                            .rounded(px(10.0))
                            .w_full(),
                    ),
            )
            .child(known_reasoning_presets(editor, cx)),
        ReasoningEditorMode::Custom => content.child(custom_reasoning_presets(editor, cx)),
    }
    .into_any_element()
}

pub(super) fn default_reasoning_action(
    id: impl Into<ElementId>,
    selected: bool,
    cx: &App,
) -> Button {
    icon_action(
        id,
        AppIcon::Pin,
        if selected {
            IconTone::Accent
        } else {
            IconTone::Muted
        },
        if selected {
            "Default preset"
        } else {
            "Set as default"
        },
        cx,
    )
    .selected(selected)
    .toggled(selected)
}

pub(super) fn known_reasoning_presets(
    editor: &ModelReasoningEditor,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let provider_default = editor.known_default == PROVIDER_DEFAULT_REASONING_PRESET;
    let mut presets = div().w_full().flex().flex_col().gap_2().child(
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
                default_reasoning_action("known-reasoning-default-provider", provider_default, cx)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_known_reasoning_default(
                            PROVIDER_DEFAULT_REASONING_PRESET.into(),
                            cx,
                        )
                    })),
            ),
    );
    for preset in &editor.known_presets {
        let level = preset.level;
        let enabled = preset.enabled;
        let selected = editor.known_default == level.as_str();
        let uses_budget = editor.format.uses_budget()
            && !matches!(
                level,
                ReasoningLevel::Off | ReasoningLevel::On | ReasoningLevel::Auto
            );
        let mut row = div()
            .rounded(px(10.0))
            .bg(cx.theme().muted)
            .px_3()
            .py_2()
            .flex()
            .items_center()
            .gap_3()
            .child(
                Switch::new(SharedString::from(format!(
                    "known-reasoning-enabled-{}",
                    level.as_str()
                )))
                .small()
                .checked(enabled)
                .color(cx.theme().primary)
                .on_click(cx.listener(move |this, value: &bool, _, cx| {
                    this.toggle_known_reasoning_preset(level, *value, cx)
                })),
            )
            .child(div().w(px(72.0)).text_sm().child(level.label()));
        if uses_budget {
            row = row.child(
                Input::new(&preset.budget_tokens)
                    .aria_label("Reasoning token budget")
                    .h(px(34.0))
                    .w(px(120.0))
                    .px_2()
                    .rounded(px(8.0)),
            );
        } else {
            row = row.child(div().flex_1());
        }
        let id = level.as_str().to_string();
        row = row.child(
            default_reasoning_action(
                SharedString::from(format!("known-reasoning-default-{id}")),
                selected,
                cx,
            )
            .disabled(!enabled)
            .on_click(
                cx.listener(move |this, _, _, cx| this.set_known_reasoning_default(id.clone(), cx)),
            ),
        );
        presets = presets.child(row);
    }
    presets.into_any_element()
}

pub(super) fn reasoning_identity_field(
    label: &'static str,
    input: &Entity<InputState>,
    cx: &App,
) -> AnyElement {
    div()
        .min_w_0()
        .flex_1()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(form_input(input, label))
        .into_any_element()
}
