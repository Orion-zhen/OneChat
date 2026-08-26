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
    let supports_reasoning = app
        .title_generation_model()
        .is_some_and(|model| model.reasoning.is_some());
    div()
        .w_full()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(
            div()
                .min_w(px(180.0))
                .flex_1()
                .child(default_model_select(app, DefaultModelRole::TitleGeneration)),
        )
        .when(supports_reasoning, |controls| {
            controls.child(
                select_control(&app.settings_ui.title_reasoning_select)
                    .placeholder("Reasoning Preset")
                    .w_auto()
                    .min_w(px(96.0))
                    .max_w(px(180.0))
                    .flex_none(),
            )
        })
        .into_any_element()
}

fn default_model_select(app: &OneChat, role: DefaultModelRole) -> AnyElement {
    match role {
        DefaultModelRole::Primary => select_control(&app.settings_ui.primary_model_select)
            .placeholder("Choose a model")
            .w_full()
            .max_w(px(340.0))
            .empty(empty_model_list)
            .into_any_element(),
        DefaultModelRole::TitleGeneration => select_control(&app.settings_ui.title_model_select)
            .placeholder("Use Current Model")
            .w_full()
            .max_w(px(340.0))
            .empty(empty_model_list)
            .into_any_element(),
    }
}

fn empty_model_list(_: &mut Window, cx: &App) -> AnyElement {
    div()
        .p_3()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child("No available models configured")
        .into_any_element()
}
