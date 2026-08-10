use super::*;

pub(super) fn reasoning_parameter_list(
    preset_index: usize,
    scope: ReasoningParameterScope,
    label: &'static str,
    parameters: &[ReasoningParameterEditor],
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let scope_id = match scope {
        ReasoningParameterScope::Request => "request",
        ReasoningParameterScope::ChatTemplateKwargs => "template",
    };
    let mut list = div().w_full().flex().flex_col().gap_2().child(
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(label),
            )
            .child(
                icon_action(
                    SharedString::from(format!("add-reasoning-{scope_id}-{preset_index}")),
                    AppIcon::Plus,
                    IconTone::Accent,
                    "Add parameter",
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.add_reasoning_parameter(preset_index, scope, window, cx)
                })),
            ),
    );
    if !parameters.is_empty() {
        list = list.child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child(div().min_w_0().flex_1().child("Path"))
                .child(div().w(px(104.0)).flex_none().child("Type"))
                .child(div().min_w_0().flex_1().child("Value"))
                .child(div().w(px(32.0)).flex_none()),
        );
    }
    list.children(
        parameters
            .iter()
            .enumerate()
            .map(|(parameter_index, parameter)| {
                let mapped_type = parameter.mapped_type(cx);
                let value_type = parameter.effective_type(cx);
                let path = match &parameter.path {
                    ReasoningParameterPathEditor::Request(input) => {
                        form_input(input, "Parameter path").into_any_element()
                    }
                    ReasoningParameterPathEditor::ChatTemplate(input) => Combobox::new(input)
                        .large()
                        .h(px(40.0))
                        .px(px(12.0))
                        .rounded(px(10.0))
                        .placeholder("Select or enter a parameter…")
                        .search_placeholder("Search or enter a parameter…")
                        .menu_max_h(px(260.0))
                        .into_any_element(),
                };
                let value_type_control = if mapped_type.is_some() {
                    div()
                        .w_full()
                        .h_full()
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded(px(10.0))
                        .bg(cx.theme().muted)
                        .text_color(cx.theme().muted_foreground)
                        .child(value_type.label())
                        .into_any_element()
                } else {
                    Select::new(&parameter.value_type)
                        .w_full()
                        .h_full()
                        .px(px(8.0))
                        .rounded(px(10.0))
                        .into_any_element()
                };
                let value = match value_type {
                    ReasoningParameterType::Boolean => Select::new(&parameter.boolean_value)
                        .w_full()
                        .h(px(40.0))
                        .px(px(12.0))
                        .rounded(px(10.0))
                        .into_any_element(),
                    ReasoningParameterType::Null => div()
                        .w_full()
                        .h(px(40.0))
                        .px_3()
                        .flex()
                        .items_center()
                        .rounded(px(10.0))
                        .bg(cx.theme().muted)
                        .text_color(cx.theme().muted_foreground)
                        .child("No value")
                        .into_any_element(),
                    _ => form_input(&parameter.value, "Parameter value").into_any_element(),
                };
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().min_w_0().flex_1().child(path))
                    .child(
                        div()
                            .w(px(104.0))
                            .h(px(40.0))
                            .flex_none()
                            .child(value_type_control),
                    )
                    .child(div().min_w_0().flex_1().child(value))
                    .child(
                        icon_action(
                            SharedString::from(format!(
                                "remove-reasoning-{scope_id}-{preset_index}-{parameter_index}"
                            )),
                            AppIcon::Trash,
                            IconTone::Danger,
                            "Remove parameter",
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove_reasoning_parameter(
                                preset_index,
                                scope,
                                parameter_index,
                                cx,
                            )
                        })),
                    )
            }),
    )
    .into_any_element()
}
