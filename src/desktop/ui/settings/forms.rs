use super::*;

pub(super) fn provider_kind_select(
    editor: &ProviderEditor,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let mut options = div()
        .w_full()
        .mt_1()
        .rounded_lg()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .p_1()
        .flex()
        .flex_col()
        .shadow_md();
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
                .children(selected.then(|| div().text_color(colors.accent).child("✓"))),
        );
    }

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
                .id("provider-kind-select")
                .w_full()
                .h(px(36.0))
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
                .child(
                    div()
                        .text_color(colors.muted)
                        .child(if editor.kind_menu_open { "⌃" } else { "⌄" }),
                ),
        )
        .children(editor.kind_menu_open.then_some(options))
        .into_any_element()
}

pub(super) fn provider_form(
    editor: &ProviderEditor,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let identity = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(field("Name", editor.name.clone(), colors))
        .child(
            div()
                .flex()
                .items_start()
                .gap_2()
                .child(provider_kind_select(editor, colors, cx))
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(colors.muted)
                                .child("Status"),
                        )
                        .child(
                            button(
                                "provider-enabled",
                                if editor.enabled {
                                    "Enabled"
                                } else {
                                    "Disabled"
                                },
                                colors,
                            )
                            .when(editor.enabled, |element| {
                                element.bg(colors.accent_soft).text_color(colors.accent)
                            })
                            .on_click(
                                cx.listener(|this, _, _, cx| this.toggle_provider_enabled(cx)),
                            ),
                        ),
                ),
        );
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
                    button("cancel-provider", "Cancel", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_provider_editor(cx))),
                )
                .child(
                    primary_button(
                        "save-provider",
                        if editor.is_new() {
                            "Add Provider"
                        } else {
                            "Save Changes"
                        },
                        colors,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.save_provider(cx))),
                ),
        )
        .into_any_element()
}

pub(super) fn model_form(
    editor: &ModelEditor,
    colors: Colors,
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
        .child(field("Remote Model ID", editor.remote_id.clone(), colors))
        .child(field("Display Name", editor.display_name.clone(), colors))
        .child(capability_group(
            "Core Capabilities",
            &Capability::CORE,
            editor,
            colors,
            cx,
        ))
        .child(capability_group(
            "Supported Parameters",
            &Capability::PARAMETERS,
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
                    button("cancel-model", "Cancel", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_model_editor(cx))),
                )
                .child(
                    primary_button("save-model", "Save Model", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.save_model(cx))),
                ),
        )
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
