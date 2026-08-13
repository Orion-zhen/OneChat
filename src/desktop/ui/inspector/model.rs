use super::*;

pub(super) fn render_model(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let Some(conversation) = app.current_conversation() else {
        return notice("Select a conversation to configure its model.", cx);
    };
    let Some(model) = app.current_model() else {
        return div()
            .flex()
            .flex_col()
            .gap_3()
            .child(notice("This conversation has no model.", cx))
            .child(
                Regular
                    .primary_icon_action(
                        "inspector-choose-model-empty",
                        AppIcon::Layers,
                        "Choose model",
                        cx,
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_model_picker(window, cx)),
                    ),
            )
            .into_any_element();
    };

    let provider = app
        .current_provider()
        .map(|provider| provider.name.as_str())
        .unwrap_or("Missing provider");
    let ignored = conversation
        .generation_config
        .filtered_for(&model.capabilities)
        .1;
    let Some(editor) = app.chat.generation_config_editor.as_ref() else {
        return notice("Opening parameter editor…", cx);
    };

    let mut parameters = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(model_summary(model, provider, cx));

    if model.reasoning.is_some() {
        parameters = parameters.child(
            div()
                .rounded(px(14.0))
                .bg(cx.theme().muted)
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Reasoning"),
                )
                .child(
                    Select::new(&editor.reasoning_select)
                        .large()
                        .h(px(40.0))
                        .px(px(12.0))
                        .rounded(px(10.0))
                        .w_full(),
                ),
        );
    }

    if !ignored.is_empty() {
        parameters = parameters.child(
            div()
                .rounded_lg()
                .bg(cx.theme().accent)
                .p_3()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                    "Not sent by this model: {}. The saved values are preserved.",
                    ignored.join(", ")
                )),
        );
    }

    let capabilities = &model.capabilities;
    for parameter in GenerationParameter::ALL {
        if editor.is_active(parameter) && parameter.supported_by(capabilities) {
            parameters = parameters.child(parameter_field(parameter, editor, cx));
        }
    }

    parameters
        .child(add_parameter_select(editor, capabilities, cx))
        .children(
            app.chat
                .parameter_error
                .as_ref()
                .map(|error| Alert::error("generation-parameter-error", error.clone()).small()),
        )
        .into_any_element()
}

fn parameter_field(
    parameter: GenerationParameter,
    editor: &GenerationConfigEditor,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    match editor.parameter_input(parameter) {
        GenerationParameterInput::Single(input) => scalar_parameter_field(parameter, &input, cx),
        GenerationParameterInput::Multiline(input) => textarea_parameter_field(
            parameter,
            &input,
            if parameter == GenerationParameter::Extra {
                140.0
            } else {
                92.0
            },
            cx,
        ),
    }
}

fn parameter_label(parameter: GenerationParameter, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(parameter.label()),
        )
        .child(
            div()
                .pt(px(2.0))
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child(parameter.hint()),
        )
}

fn remove_parameter_button(parameter: GenerationParameter, cx: &mut Context<OneChat>) -> Button {
    Button::new(SharedString::from(format!(
        "remove-parameter-{}",
        parameter.id()
    )))
    .ghost()
    .tooltip("Remove parameter")
    .size(px(30.0))
    .p_0()
    .child(render_icon(AppIcon::Close, IconTone::Muted, 15.0, cx))
    .on_click(cx.listener(move |this, _, window, cx| {
        this.remove_generation_parameter(parameter, window, cx)
    }))
}

fn scalar_parameter_field(
    parameter: GenerationParameter,
    input: &Entity<InputState>,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .rounded(px(14.0))
        .bg(cx.theme().muted)
        .p_3()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(parameter_label(parameter, cx)),
        )
        .child(
            Input::new(input)
                .aria_label(parameter.label())
                .w(px(112.0))
                .h(px(40.0))
                .px_3()
                .rounded(px(10.0))
                .text_right(),
        )
        .child(remove_parameter_button(parameter, cx))
        .into_any_element()
}

fn textarea_parameter_field(
    parameter: GenerationParameter,
    input: &Entity<TextareaState>,
    height: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .rounded(px(14.0))
        .bg(cx.theme().muted)
        .p_3()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap_2()
                .child(parameter_label(parameter, cx))
                .child(remove_parameter_button(parameter, cx)),
        )
        .child(
            Textarea::new(input)
                .aria_label(parameter.label())
                .h(px(height))
                .rounded(px(10.0)),
        )
        .into_any_element()
}

fn add_parameter_select(
    editor: &GenerationConfigEditor,
    capabilities: &crate::domain::ModelCapabilities,
    cx: &App,
) -> AnyElement {
    let disabled = GenerationParameter::ALL
        .into_iter()
        .all(|parameter| !parameter.supported_by(capabilities) || editor.is_active(parameter));
    Select::new(&editor.parameter_select)
        .large()
        .h(px(44.0))
        .px(px(14.0))
        .rounded(px(12.0))
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .placeholder(if disabled {
            "All available parameters added"
        } else {
            "Add parameter"
        })
        .disabled(disabled)
        .w_full()
        .into_any_element()
}

fn model_summary(model: &Model, provider: &str, cx: &App) -> AnyElement {
    let metadata = crate::desktop::ui::model::capability_summary(model, " · ");
    let details = if metadata.is_empty() {
        provider.to_string()
    } else {
        format!("{provider} · {metadata}")
    };
    div()
        .rounded_lg()
        .bg(cx.theme().muted)
        .p_3()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child("Model"),
        )
        .child(
            div()
                .pt_1()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(model.display_name.clone()),
        )
        .child(
            div()
                .pt_1()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child(details),
        )
        .into_any_element()
}
