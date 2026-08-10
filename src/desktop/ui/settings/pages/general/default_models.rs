use super::*;

pub(in crate::desktop::ui::settings) fn default_models_page(
    app: &OneChat,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let content = div()
        .w_full()
        .flex()
        .flex_col()
        .child(setting_row(
            "Primary Model",
            "Used when creating a new conversation.",
            default_model_select(app, DefaultModelRole::Primary),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row(
            "Title Generation Model",
            "Used for automatic titles after the first response.",
            title_generation_controls(app),
            cx,
        ));

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "Default Models",
                "Choose the models OneChat uses by default.",
                cx,
            ))
            .child(section(
                "Model Selection",
                Some("Only models that are ready to use appear here."),
                content,
                cx,
            )),
    )
}

fn title_generation_controls(app: &OneChat) -> AnyElement {
    let reasoning_menu_width = app
        .title_generation_model()
        .and_then(|model| model.reasoning.as_ref())
        .map(|reasoning| {
            reasoning
                .preset_options()
                .iter()
                .map(|(_, label)| {
                    label
                        .chars()
                        .map(|character| if character.is_ascii() { 8.0 } else { 16.0 })
                        .sum::<f32>()
                        + 56.0
                })
                .fold(96.0, f32::max)
                .min(280.0)
        });
    let supports_reasoning = reasoning_menu_width.is_some();
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(default_model_select(app, DefaultModelRole::TitleGeneration))
        .when(supports_reasoning, |controls| {
            controls.child(
                Select::new(&app.settings_ui.title_reasoning_select)
                    .large()
                    .h(px(40.0))
                    .px(px(12.0))
                    .rounded(px(10.0))
                    .placeholder("Reasoning Preset")
                    .menu_width(px(reasoning_menu_width.unwrap_or(96.0)))
                    .menu_max_h(px(320.0))
                    .w_auto()
                    .min_w(px(96.0))
                    .max_w(px(180.0))
                    .flex_none(),
            )
        })
        .into_any_element()
}

fn default_model_select(app: &OneChat, role: DefaultModelRole) -> AnyElement {
    let state = match role {
        DefaultModelRole::Primary => &app.settings_ui.primary_model_select,
        DefaultModelRole::TitleGeneration => &app.settings_ui.title_model_select,
    };
    Select::new(state)
        .large()
        .h(px(40.0))
        .px(px(12.0))
        .rounded(px(10.0))
        .placeholder(match role {
            DefaultModelRole::Primary => "Choose a model",
            DefaultModelRole::TitleGeneration => "Use Primary Model",
        })
        .menu_max_h(px(320.0))
        .w(px(300.0))
        .empty(|_, cx| {
            div()
                .p_3()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("No available models configured")
        })
        .into_any_element()
}
