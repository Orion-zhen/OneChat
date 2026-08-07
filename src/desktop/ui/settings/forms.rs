use super::*;

pub(super) fn provider_kind_select(
    editor: &ProviderEditor,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let mut options = div()
        .id("provider-kind-options")
        .occlude()
        .absolute()
        .top(px(54.0))
        .left_0()
        .right_0()
        .rounded_lg()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .p_1()
        .flex()
        .flex_col()
        .shadow_lg();
    for kind in ProviderKind::ALL {
        let selected = kind == editor.kind;
        options = options.child(
            div()
                .id(SharedString::from(format!(
                    "provider-kind-option-{}",
                    kind.as_str()
                )))
                .w_full()
                .px_3()
                .py_2()
                .rounded_md()
                .flex()
                .items_center()
                .justify_between()
                .bg(if selected {
                    colors.accent_soft
                } else {
                    colors.panel
                })
                .text_sm()
                .text_color(if selected { colors.accent } else { colors.text })
                .cursor_pointer()
                .hover(move |style| style.bg(colors.hover))
                .on_click(cx.listener(move |this, _, _, cx| this.select_provider_kind(kind, cx)))
                .child(kind.label())
                .children(selected.then(|| {
                    render_icon(Icon::Check, IconTone::Accent, colors, scale_factor, 14.0)
                })),
        );
    }

    let select = div()
        .id("provider-kind-select")
        .w_full()
        .h(px(50.0))
        .px_3()
        .rounded_lg()
        .border_1()
        .border_color(colors.border)
        .bg(colors.raised)
        .flex()
        .items_center()
        .justify_between()
        .text_sm()
        .cursor_pointer()
        .hover(move |style| style.bg(colors.hover))
        .on_click(cx.listener(|this, _, _, cx| this.toggle_provider_kind_menu(cx)))
        .child(editor.kind.label())
        .child(render_icon(
            if editor.kind_menu_open {
                Icon::ChevronUp
            } else {
                Icon::ChevronDown
            },
            IconTone::Muted,
            colors,
            scale_factor,
            14.0,
        ));

    div()
        .min_w(px(240.0))
        .flex_1()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.muted)
                .child("Type"),
        )
        .child(
            div()
                .relative()
                .w_full()
                .child(select)
                .children(editor.kind_menu_open.then(|| deferred(options).priority(1))),
        )
        .into_any_element()
}

pub(super) fn provider_form(
    editor: &ProviderEditor,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let identity = div()
        .flex()
        .items_start()
        .gap_2()
        .child(
            div()
                .min_w(px(240.0))
                .flex_1()
                .child(field("Name", editor.name.clone(), colors)),
        )
        .child(provider_kind_select(editor, colors, scale_factor, cx));
    let connection = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(field("Endpoint", editor.endpoint.clone(), colors))
        .child(field(
            "API Key · stored as plain text",
            editor.api_key.clone(),
            colors,
        ));
    let advanced = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(field(
            "Custom Headers · JSON",
            editor.headers.clone(),
            colors,
        ))
        .child(field("Proxy", editor.proxy.clone(), colors));

    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(section("Provider", None, identity, colors))
        .child(section("Connection", None, connection, colors))
        .child(section(
            "Advanced",
            Some("Optional request headers and proxy routing."),
            advanced,
            colors,
        ))
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    large_icon_button(
                        "cancel-provider",
                        Icon::Close,
                        IconTone::Muted,
                        colors,
                        scale_factor,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_provider_editor(cx))),
                )
                .child(
                    primary_icon_button("save-provider", Icon::Save, colors, scale_factor)
                        .on_click(cx.listener(|this, _, _, cx| this.save_provider(cx))),
                ),
        )
        .into_any_element()
}

pub(super) fn model_form(
    editor: &ModelEditor,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let title = if editor.is_new() {
        "Add Model"
    } else {
        "Edit Model"
    };
    div()
        .rounded_lg()
        .bg(colors.raised)
        .p_4()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(model_id_combobox(editor, colors, scale_factor, cx))
        .child(field("Display Name", editor.display_name.clone(), colors))
        .child(capability_group(
            "Core Capabilities",
            &Capability::CORE,
            editor,
            colors,
            cx,
        ))
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    large_icon_button(
                        "cancel-model",
                        Icon::Close,
                        IconTone::Muted,
                        colors,
                        scale_factor,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_model_editor(cx))),
                )
                .child(
                    primary_icon_button("save-model", Icon::Save, colors, scale_factor)
                        .on_click(cx.listener(|this, _, _, cx| this.save_model(cx))),
                ),
        )
        .into_any_element()
}

fn model_id_combobox(
    editor: &ModelEditor,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let control = div()
        .w_full()
        .flex()
        .gap_1()
        .child(div().min_w_0().flex_1().child(editor.remote_id.clone()))
        .child(
            icon_button(
                "available-model-menu",
                if editor.model_menu_open {
                    Icon::ChevronUp
                } else {
                    Icon::ChevronDown
                },
                IconTone::Muted,
                colors,
                scale_factor,
            )
            .size(px(50.0))
            .rounded_lg()
            .border_1()
            .border_color(colors.border)
            .bg(colors.raised)
            .on_click(cx.listener(|this, _, _, cx| this.toggle_available_model_menu(cx))),
        );

    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.muted)
                .child("Model ID"),
        )
        .child(
            div().relative().w_full().child(control).children(
                editor
                    .model_menu_open
                    .then(|| deferred(available_model_menu(editor, colors, cx)).priority(1)),
            ),
        )
        .into_any_element()
}

fn available_model_menu(
    editor: &ModelEditor,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let content = match &editor.fetch_status {
        ModelFetchStatus::Loading => div()
            .px_3()
            .py_2()
            .text_sm()
            .text_color(colors.muted)
            .child("Loading available models…")
            .into_any_element(),
        ModelFetchStatus::Failed(error) => div()
            .px_3()
            .py_2()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .min_w_0()
                    .text_sm()
                    .text_color(colors.danger)
                    .child(error.clone()),
            )
            .child(
                compact_button("retry-model-list", "Retry", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.retry_available_models(cx))),
            )
            .into_any_element(),
        ModelFetchStatus::Loaded if editor.available_models.is_empty() => div()
            .px_3()
            .py_2()
            .text_sm()
            .text_color(colors.muted)
            .child("No unconfigured models were returned. You can enter an ID manually.")
            .into_any_element(),
        ModelFetchStatus::Loaded => {
            let visible = editor.visible_models(cx);
            if visible.is_empty() {
                div()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(colors.muted)
                    .child("No matching models. You can use the entered ID.")
                    .into_any_element()
            } else {
                let mut options = div().flex().flex_col().p_1();
                for (index, model) in visible.into_iter().enumerate() {
                    let remote_id = model.id.clone();
                    let selected = index == editor.model_selection;
                    options = options.child(
                        div()
                            .id(SharedString::from(format!("available-model-{remote_id}")))
                            .w_full()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .bg(if selected {
                                colors.accent_soft
                            } else {
                                colors.panel
                            })
                            .text_sm()
                            .text_color(if selected { colors.accent } else { colors.text })
                            .cursor_pointer()
                            .hover(move |style| style.bg(colors.hover))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_available_model(remote_id.clone(), cx)
                            }))
                            .child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .child(model.id.clone()),
                            )
                            .children(model.vision.then(|| {
                                div()
                                    .flex_none()
                                    .text_size(px(11.0))
                                    .text_color(colors.muted)
                                    .child("Vision")
                            })),
                    );
                }
                options.into_any_element()
            }
        }
    };

    div()
        .id("available-model-options")
        .occlude()
        .absolute()
        .top(px(54.0))
        .left_0()
        .right_0()
        .max_h(px(260.0))
        .overflow_y_scroll()
        .rounded_lg()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .shadow_lg()
        .child(content)
        .into_any_element()
}

fn capability_group(
    title: &'static str,
    capabilities: &'static [Capability],
    editor: &ModelEditor,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let mut toggles = div().flex().flex_wrap().gap_2();
    for capability in capabilities {
        let capability = *capability;
        let enabled = editor.capability(capability);
        toggles = toggles.child(
            button(
                SharedString::from(format!("capability-{capability:?}")),
                capability.label(),
                colors,
            )
            .when(enabled, |element| {
                element.bg(colors.accent_soft).text_color(colors.accent)
            })
            .when(!enabled, |element| element.text_color(colors.muted))
            .on_click(
                cx.listener(move |this, _, _, cx| this.toggle_model_capability(capability, cx)),
            ),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.muted)
                .child(title),
        )
        .child(toggles)
        .into_any_element()
}
