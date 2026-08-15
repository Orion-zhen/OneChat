use super::*;

pub(in crate::desktop::ui::settings) fn provider_kind_select(
    editor: &ProviderEditor,
) -> AnyElement {
    field_control(Select::new(&editor.kind))
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
                .child(input_with_error(
                    &editor.name,
                    "Provider name",
                    editor.errors.name.as_deref(),
                    cx,
                )),
        )
        .child(
            Field::new()
                .label("Type")
                .required(true)
                .child(provider_kind_select(editor)),
        );
    let streaming = editor.streaming;
    let connection_fields = Form::vertical()
        .columns(5)
        .child(
            Field::new()
                .label("Endpoint")
                .required(true)
                .col_span(4)
                .child(input_with_error(
                    &editor.endpoint,
                    "Provider endpoint",
                    editor.errors.endpoint.as_deref(),
                    cx,
                )),
        )
        .child(
            Field::new().label("Capabilities").child(
                div()
                    .w_full()
                    .min_h(px(40.0))
                    .px_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(cx.theme().muted_foreground)
                            .child("Streaming"),
                    )
                    .child(
                        Switch::new("provider-capability-streaming")
                            .small()
                            .checked(streaming)
                            .color(cx.theme().primary)
                            .tooltip(if streaming {
                                "Disable streaming"
                            } else {
                                "Enable streaming"
                            })
                            .on_click(cx.listener(move |this, enabled: &bool, _, cx| {
                                this.set_provider_streaming(*enabled, cx)
                            })),
                    ),
            ),
        )
        .child(
            Field::new().label("API Key").col_span(5).child(
                form_input(&editor.api_key, "API key")
                    .content_type(InputContentType::Password)
                    .mask_toggle(),
            ),
        );
    let connection = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(connection_fields)
        .children(provider_test_status(editor, cx));

    let headers =
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .children(editor.headers.iter().enumerate().map(|(index, header)| {
                let is_draft = index + 1 == editor.headers.len();
                let add_disabled = header.name.read(cx).value().trim().is_empty();
                let action =
                    if is_draft {
                        Compact
                            .icon_action(
                                "add-provider-header",
                                AppIcon::Plus,
                                IconTone::Accent,
                                "Add custom header",
                                cx,
                            )
                            .disabled(add_disabled)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.add_provider_header(window, cx)
                            }))
                    } else {
                        Compact
                            .icon_action(
                                SharedString::from(format!("remove-provider-header-{index}")),
                                AppIcon::Trash,
                                IconTone::Danger,
                                "Remove custom header",
                                cx,
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.remove_provider_header(index, cx)
                            }))
                    };
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(key_value_input_row(
                        form_input(&header.name, "Custom header name"),
                        form_input(&header.value, "Custom header value"),
                        action,
                    ))
                    .children(
                        editor
                            .errors
                            .headers
                            .get(&index)
                            .map(|error| validation_error(error, cx)),
                    )
            }));
    let advanced = Form::vertical()
        .child(
            Field::new()
                .label("Proxy")
                .description("Optional HTTP or SOCKS proxy URL")
                .child(input_with_error(
                    &editor.proxy,
                    "Optional proxy URL",
                    editor.errors.proxy.as_deref(),
                    cx,
                )),
        )
        .child(
            Field::new()
                .label("Custom Headers")
                .description("Header name and value pairs added to every request")
                .child(headers),
        );

    div()
        .on_action(
            cx.listener(|this, _: &InputEscape, window, cx| {
                this.cancel_provider_editor(window, cx)
            }),
        )
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
        .into_any_element()
}

pub(in crate::desktop::ui::settings) fn provider_form_actions(
    editor: &ProviderEditor,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let testing = matches!(editor.test_status(cx), Some(ConnectionTestStatus::Testing));
    let busy = editor.saving || testing;
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap_2()
        .child(
            Compact
                .icon_action(
                    "cancel-provider",
                    AppIcon::Close,
                    IconTone::Muted,
                    "Cancel editing (Esc)",
                    cx,
                )
                .disabled(editor.saving)
                .on_click(
                    cx.listener(|this, _, window, cx| this.cancel_provider_editor(window, cx)),
                ),
        )
        .child(
            Compact
                .icon_action(
                    "test-provider-editor",
                    AppIcon::Plug,
                    IconTone::Accent,
                    "Test connection",
                    cx,
                )
                .loading(testing)
                .disabled(busy)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.test_provider_editor_connection(window, cx)
                })),
        )
        .child(
            Compact
                .primary_icon_action(
                    "save-provider",
                    AppIcon::Save,
                    provider_save_tooltip(editor.is_new()),
                    cx,
                )
                .loading(editor.saving)
                .disabled(busy || !editor.is_dirty(cx))
                .on_click(cx.listener(|this, _, window, cx| this.save_provider(window, cx))),
        )
        .into_any_element()
}

fn provider_save_tooltip(is_new: bool) -> &'static str {
    match (is_new, cfg!(target_os = "macos")) {
        (true, true) => "Add provider (⌘S)",
        (false, true) => "Save changes (⌘S)",
        (true, false) => "Add provider (Ctrl+S)",
        (false, false) => "Save changes (Ctrl+S)",
    }
}

fn input_with_error(
    state: &Entity<InputState>,
    label: &'static str,
    error: Option<&str>,
    cx: &App,
) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_1()
        .child(form_input(state, label))
        .children(error.map(|error| validation_error(error, cx)))
        .into_any_element()
}

fn validation_error(error: &str, cx: &App) -> AnyElement {
    div()
        .px_1()
        .text_size(px(11.0))
        .line_height(px(16.0))
        .text_color(cx.theme().danger)
        .child(error.to_string())
        .into_any_element()
}

fn provider_test_status(editor: &ProviderEditor, cx: &App) -> Option<AnyElement> {
    match editor.test_status(cx)? {
        ConnectionTestStatus::Testing => Some(
            div()
                .rounded_lg()
                .bg(cx.theme().muted)
                .p_3()
                .flex()
                .items_center()
                .gap_2()
                .child(Spinner::new().small().color(cx.theme().primary))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child("Testing connection…"),
                )
                .into_any_element(),
        ),
        ConnectionTestStatus::Connected => Some(
            Alert::success("provider-editor-test-success", "Connection succeeded")
                .small()
                .into_any_element(),
        ),
        ConnectionTestStatus::Failed(message) => Some(
            Alert::error("provider-editor-test-error", message.clone())
                .small()
                .into_any_element(),
        ),
    }
}
