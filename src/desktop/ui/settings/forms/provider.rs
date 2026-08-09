use super::*;

pub(in crate::desktop::ui::settings) fn provider_kind_select(
    editor: &ProviderEditor,
) -> AnyElement {
    Select::new(&editor.kind)
        .large()
        .h(px(40.0))
        .px(px(12.0))
        .rounded(px(10.0))
        .placeholder("Provider type")
        .w_full()
        .into_any_element()
}

pub(in crate::desktop::ui::settings) fn provider_form(
    editor: &ProviderEditor,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let identity = Form::vertical()
        .columns(2)
        .child(
            Field::new()
                .label("Name")
                .required(true)
                .child(form_input(&editor.name, "Provider name")),
        )
        .child(
            Field::new()
                .label("Type")
                .required(true)
                .child(provider_kind_select(editor)),
        );
    let connection = Form::vertical()
        .child(
            Field::new()
                .label("Endpoint")
                .required(true)
                .child(form_input(&editor.endpoint, "Provider endpoint")),
        )
        .child(
            Field::new().label("API Key").child(
                form_input(&editor.api_key, "API key")
                    .content_type(InputContentType::Password)
                    .mask_toggle(),
            ),
        );
    let headers =
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .children(editor.headers.iter().enumerate().map(|(index, header)| {
                let is_draft = index + 1 == editor.headers.len();
                let action = if is_draft {
                    icon_action(
                        "add-provider-header",
                        AppIcon::Plus,
                        IconTone::Accent,
                        "Add custom header",
                        cx,
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| this.add_provider_header(window, cx)),
                    )
                } else {
                    icon_action(
                        SharedString::from(format!("remove-provider-header-{index}")),
                        AppIcon::Trash,
                        IconTone::Danger,
                        "Remove custom header",
                        cx,
                    )
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.remove_provider_header(index, cx)),
                    )
                };
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(form_input(&header.name, "Custom header name")),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(form_input(&header.value, "Custom header value")),
                    )
                    .child(action)
            }));
    let advanced = Form::vertical()
        .child(
            Field::new()
                .label("Proxy")
                .description("Optional HTTP or SOCKS proxy URL")
                .child(form_input(&editor.proxy, "Optional proxy URL")),
        )
        .child(
            Field::new()
                .label("Custom Headers")
                .description("Header name and value pairs added to every request")
                .child(headers),
        );

    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(section("Provider", None, identity, cx))
        .child(section(
            "Connection",
            Some("Credentials are stored as plain text on this Mac."),
            connection,
            cx,
        ))
        .child(section(
            "Advanced",
            Some("Optional request headers and proxy routing."),
            advanced,
            cx,
        ))
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    icon_action(
                        "cancel-provider",
                        AppIcon::Close,
                        IconTone::Muted,
                        "Cancel",
                        cx,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_provider_editor(cx))),
                )
                .child(
                    primary_icon_action("save-provider", AppIcon::Save, "Save provider", cx)
                        .on_click(cx.listener(|this, _, _, cx| this.save_provider(cx))),
                ),
        )
        .into_any_element()
}
