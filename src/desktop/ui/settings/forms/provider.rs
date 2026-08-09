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
                .description("Optional JSON object added to every request")
                .child(
                    Input::new(&editor.headers)
                        .large()
                        .aria_label("Custom headers JSON")
                        .h(px(104.0)),
                ),
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
