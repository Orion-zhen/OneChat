use super::*;

pub(super) fn new_provider_page(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let content = app.settings_ui.provider_editor.as_ref().map_or_else(
        || {
            div()
                .text_sm()
                .text_color(colors.muted)
                .child("Preparing provider settings…")
                .into_any_element()
        },
        |editor| provider_form(editor, colors, scale_factor, cx),
    );

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "Add Provider",
                "Connect OneChat to an LLM service.",
                colors,
            ))
            .children(
                app.settings_ui
                    .form_error
                    .as_ref()
                    .map(|error| error_banner(error, colors)),
            )
            .child(content),
    )
}

pub(super) fn provider_page(
    app: &OneChat,
    provider: &Provider,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let (status, status_color) = provider_status(app, provider, colors);
    let provider_id = provider.id.clone();
    let toggle_id = provider.id.clone();
    let edit_id = provider.id.clone();
    let testing = matches!(
        app.settings_ui.connection_tests.get(&provider.id),
        Some(ConnectionTestStatus::Testing)
    );
    let header = div()
        .flex()
        .items_start()
        .justify_between()
        .gap_5()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .text_size(px(28.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(provider.name.clone()),
                )
                .child(
                    div()
                        .pt_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_sm()
                        .text_color(colors.muted)
                        .child(div().size(px(7.0)).rounded_full().bg(status_color))
                        .child(format!("{} · {status}", provider.kind.label())),
                ),
        )
        .when(app.settings_ui.provider_editor.is_none(), |element| {
            element.child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id(SharedString::from(format!(
                                "toggle-provider-{}",
                                provider.id
                            )))
                            .w(px(32.0))
                            .h(px(18.0))
                            .p(px(2.0))
                            .rounded_full()
                            .border_1()
                            .border_color(if provider.enabled {
                                colors.accent
                            } else {
                                colors.border
                            })
                            .bg(if provider.enabled {
                                colors.accent
                            } else {
                                colors.raised
                            })
                            .flex()
                            .items_center()
                            .when(provider.enabled, |element| element.justify_end())
                            .cursor_pointer()
                            .hover(|style| style.opacity(0.8))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_provider_enabled(toggle_id.clone(), cx)
                            }))
                            .child(div().size(px(12.0)).rounded_full().bg(if provider.enabled {
                                colors.on_accent
                            } else {
                                colors.muted
                            })),
                    )
                    .child(
                        button(
                            SharedString::from(format!("test-provider-{}", provider.id)),
                            if testing {
                                "Testing…"
                            } else {
                                "Test Connection"
                            },
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.test_provider_connection(provider_id.clone(), cx)
                        })),
                    )
                    .child(
                        primary_button(
                            SharedString::from(format!("edit-provider-{}", provider.id)),
                            "Edit",
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.begin_edit_provider(edit_id.clone(), cx)
                        })),
                    ),
            )
        });

    let body = if let Some(editor) = &app.settings_ui.provider_editor {
        provider_form(editor, colors, scale_factor, cx)
    } else {
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(provider_summary(provider, colors))
            .child(provider_models(app, provider, colors, scale_factor, cx))
            .child(provider_danger_zone(provider, colors, cx))
            .into_any_element()
    };

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(header)
            .children(
                app.settings_ui
                    .form_error
                    .as_ref()
                    .map(|error| error_banner(error, colors)),
            )
            .child(body),
    )
}

fn provider_status(app: &OneChat, provider: &Provider, colors: Colors) -> (String, gpui::Rgba) {
    match app.settings_ui.connection_tests.get(&provider.id) {
        Some(ConnectionTestStatus::Testing) => ("Testing connection…".into(), colors.accent),
        Some(ConnectionTestStatus::Connected) => ("Connected".into(), colors.success),
        Some(ConnectionTestStatus::Failed(message)) => {
            (format!("Connection failed: {message}"), colors.danger)
        }
        None if provider.enabled => ("Enabled".into(), colors.success),
        None => ("Disabled".into(), colors.muted),
    }
}

fn provider_summary(provider: &Provider, colors: Colors) -> AnyElement {
    let api_key = if provider.api_key.is_empty() {
        "Not configured"
    } else {
        "Configured"
    };
    let proxy = provider.proxy.clone().unwrap_or_else(|| "None".into());
    let headers = match provider.headers.len() {
        0 => "None".to_string(),
        1 => "1 custom header".to_string(),
        count => format!("{count} custom headers"),
    };
    let content = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(summary_row("Endpoint", provider.endpoint.clone(), colors))
        .child(summary_row("API Key", api_key, colors))
        .child(summary_row("Custom Headers", headers, colors))
        .child(summary_row("Proxy", proxy, colors));
    section(
        "Connection",
        Some("Credentials are stored as plain text on this Mac."),
        content,
        colors,
    )
}

fn provider_danger_zone(
    provider: &Provider,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let provider_id = provider.id.clone();
    let content = div()
        .flex()
        .items_center()
        .justify_between()
        .gap_5()
        .child(
            div()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Delete Provider"),
                )
                .child(
                    div()
                        .pt_1()
                        .text_size(px(12.0))
                        .text_color(colors.muted)
                        .child("This also removes every model configured for this provider."),
                ),
        )
        .child(
            button("delete-provider", "Delete…", colors)
                .text_color(colors.danger)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.request_delete_provider(provider_id.clone(), cx)
                })),
        );
    section("Danger Zone", None, content, colors)
}

fn provider_models(
    app: &OneChat,
    provider: &Provider,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let editor = app
        .settings_ui
        .model_editor
        .as_ref()
        .filter(|editor| editor.provider_id == provider.id);
    let editing_id = editor.and_then(ModelEditor::editing_id);
    let provider_id = provider.id.clone();
    let mut models = div().flex().flex_col().gap_2();

    if let Some(editor) = editor {
        models = models.child(model_form(editor, colors, scale_factor, cx));
    }

    let configured_models = app
        .data
        .snapshot
        .models
        .iter()
        .filter(|model| model.provider_id == provider.id)
        .filter(|model| editing_id != Some(model.id.as_str()))
        .collect::<Vec<_>>();

    if configured_models.is_empty() && editor.is_none() {
        models = models.child(
            div()
                .rounded_lg()
                .bg(colors.raised)
                .p_5()
                .text_center()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("No models yet"),
                )
                .child(
                    div()
                        .pt_1()
                        .text_size(px(12.0))
                        .text_color(colors.muted)
                        .child("Add a remote model ID to use this provider in conversations."),
                ),
        );
    }

    for model in configured_models {
        models = models.child(model_row(model, colors, cx));
    }

    let header = div()
        .flex()
        .items_end()
        .justify_between()
        .gap_4()
        .child(
            div()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Models"),
                )
                .child(
                    div()
                        .pt_1()
                        .text_size(px(12.0))
                        .text_color(colors.muted)
                        .child("Models are configured and managed within this provider."),
                ),
        )
        .when(editor.is_none(), |element| {
            element.child(primary_button("add-model", "Add Model", colors).on_click(
                cx.listener(move |this, _, _, cx| this.begin_add_model(provider_id.clone(), cx)),
            ))
        });

    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(header)
        .child(
            div()
                .rounded_xl()
                .border_1()
                .border_color(colors.border)
                .bg(colors.panel)
                .p_4()
                .child(models),
        )
        .into_any_element()
}

fn model_row(model: &Model, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let edit_id = model.id.clone();
    let delete_id = model.id.clone();
    div()
        .rounded_lg()
        .bg(colors.raised)
        .px_4()
        .py_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(model.display_name.clone()),
                )
                .child(
                    div()
                        .pt_1()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(11.0))
                        .text_color(colors.muted)
                        .child(format!(
                            "{} · {}",
                            model.remote_id,
                            model_capability_summary(&model.capabilities)
                        )),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .gap_1()
                .child(
                    compact_button(
                        SharedString::from(format!("edit-model-{}", model.id)),
                        "Edit",
                        colors,
                    )
                    .on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.begin_edit_model(edit_id.clone(), cx)
                        }),
                    ),
                )
                .child(
                    compact_button(
                        SharedString::from(format!("delete-model-{}", model.id)),
                        "Delete",
                        colors,
                    )
                    .text_color(colors.danger)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request_delete_model(delete_id.clone(), cx)
                    })),
                ),
        )
        .into_any_element()
}

pub(super) fn model_capability_summary(capabilities: &ModelCapabilities) -> String {
    let mut labels = Vec::new();
    if capabilities.streaming {
        labels.push("Streaming");
    }
    if capabilities.vision {
        labels.push("Vision");
    }
    if capabilities.thinking {
        labels.push("Thinking");
    }
    if labels.is_empty() {
        "No core capabilities".into()
    } else {
        labels.join(", ")
    }
}
