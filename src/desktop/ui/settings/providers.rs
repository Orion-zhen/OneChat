use super::*;

pub(super) fn new_provider_page(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let Some(editor) = &app.settings_ui.provider_editor else {
        return detail_page(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("Preparing provider settings…"),
        );
    };
    let header = div()
        .flex()
        .items_start()
        .justify_between()
        .gap_5()
        .child(div().min_w_0().flex_1().child(page_header(
            "Add Provider",
            "Connect OneChat to an LLM service.",
            cx,
        )))
        .child(provider_form_actions(editor, cx));
    let content = div()
        .flex()
        .flex_col()
        .gap_6()
        .child(header)
        .children(
            app.settings_ui
                .form_error
                .as_ref()
                .map(|error| error_banner(error)),
        )
        .child(provider_form(editor, cx));
    detail_page(content)
}

pub(super) fn provider_page(
    app: &OneChat,
    provider: &Provider,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let (saved_status, saved_status_color) = provider_status(app, provider, cx);
    let editing = app.settings_ui.provider_editor.as_ref();
    let (title, status, status_color) = if let Some(editor) = editing {
        (
            format!("Edit “{}”", provider.name),
            if editor.is_dirty(cx) {
                "Unsaved changes".to_string()
            } else {
                saved_status
            },
            if editor.is_dirty(cx) {
                cx.theme().muted_foreground
            } else {
                saved_status_color
            },
        )
    } else {
        (provider.name.clone(), saved_status, saved_status_color)
    };
    let provider_id = provider.id.clone();
    let edit_id = provider.id.clone();
    let testing = matches!(
        app.settings_ui.connection_tests.get(&provider.id),
        Some(ConnectionTestStatus::Testing)
    );
    let header_actions = if let Some(editor) = editing {
        provider_form_actions(editor, cx)
    } else {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .child(
                icon_action(
                    SharedString::from(format!("test-provider-{}", provider.id)),
                    AppIcon::Plug,
                    IconTone::Muted,
                    "Test connection",
                    cx,
                )
                .loading(testing)
                .disabled(testing)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.test_provider_connection(provider_id.clone(), cx)
                })),
            )
            .child(
                primary_icon_action(
                    SharedString::from(format!("edit-provider-{}", provider.id)),
                    AppIcon::Pencil,
                    "Edit provider",
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.begin_edit_provider(edit_id.clone(), window, cx)
                })),
            )
            .into_any_element()
    };
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
                        .child(title),
                )
                .child(
                    div()
                        .pt_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            div()
                                .size(px(6.0))
                                .flex_none()
                                .rounded_full()
                                .bg(status_color),
                        )
                        .child(format!("{} · {status}", provider.kind.label())),
                ),
        )
        .child(header_actions);

    let body = if let Some(editor) = &app.settings_ui.provider_editor {
        provider_form(editor, cx)
    } else {
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(provider_summary(provider, cx))
            .child(provider_models(app, provider, cx))
            .child(provider_danger_zone(provider, cx))
            .into_any_element()
    };

    let content = div()
        .flex()
        .flex_col()
        .gap_6()
        .child(header)
        .children(
            app.settings_ui
                .form_error
                .as_ref()
                .map(|error| error_banner(error)),
        )
        .children(
            app.settings_ui
                .connection_tests
                .get(&provider.id)
                .and_then(|status| match status {
                    ConnectionTestStatus::Failed(message) => Some(
                        Alert::error("provider-connection-error", message.clone())
                            .into_any_element(),
                    ),
                    ConnectionTestStatus::Connected => Some(
                        Alert::success("provider-connection-success", "Connection succeeded")
                            .into_any_element(),
                    ),
                    ConnectionTestStatus::Testing => None,
                }),
        )
        .child(body);
    detail_page(content)
}

fn provider_status(app: &OneChat, provider: &Provider, cx: &App) -> (String, gpui::Hsla) {
    match app.settings_ui.connection_tests.get(&provider.id) {
        Some(ConnectionTestStatus::Testing) => ("Testing connection…".into(), cx.theme().primary),
        Some(ConnectionTestStatus::Connected) => ("Connected".into(), cx.theme().success),
        Some(ConnectionTestStatus::Failed(message)) => {
            (format!("Connection failed: {message}"), cx.theme().danger)
        }
        None if provider.enabled => ("Enabled".into(), cx.theme().success),
        None => ("Disabled".into(), cx.theme().muted_foreground),
    }
}

fn provider_summary(provider: &Provider, cx: &App) -> AnyElement {
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
        .w_full()
        .flex()
        .flex_col()
        .child(provider_endpoint_row(provider, cx))
        .child(setting_divider(cx))
        .child(summary_row("API Key", api_key, cx))
        .child(setting_divider(cx))
        .child(summary_row("Custom Headers", headers, cx))
        .child(setting_divider(cx))
        .child(summary_row("Proxy", proxy, cx));
    section(
        "Connection",
        Some("Credentials are stored as plain text on this Mac."),
        content,
        cx,
    )
}

fn provider_endpoint_row(provider: &Provider, cx: &App) -> AnyElement {
    div()
        .w_full()
        .min_h(px(56.0))
        .rounded(px(10.0))
        .bg(cx.theme().transparent)
        .px_4()
        .py_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_5()
        .child(
            div()
                .flex_none()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Endpoint"),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(provider.endpoint.clone()),
        )
        .children(provider.streaming.then(|| {
            div()
                .flex_none()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().primary)
                .child("Streaming")
        }))
        .into_any_element()
}

fn provider_danger_zone(provider: &Provider, cx: &mut Context<OneChat>) -> AnyElement {
    let provider_id = provider.id.clone();
    let delete = danger_icon_action("delete-provider", AppIcon::Trash, "Delete provider", cx)
        .on_click(cx.listener(move |this, _, window, cx| {
            this.request_delete_provider(provider_id.clone(), window, cx)
        }));
    let content = setting_row(
        "Delete Provider",
        "Also removes every model configured for this provider.",
        delete,
        cx,
    );
    section("Danger Zone", None, content, cx)
}

fn provider_models(app: &OneChat, provider: &Provider, cx: &mut Context<OneChat>) -> AnyElement {
    let editor = app
        .settings_ui
        .model_editor
        .as_ref()
        .filter(|editor| editor.provider_id == provider.id);
    let editing_id = editor.and_then(ModelEditor::editing_id);
    let configured_models = app
        .data
        .snapshot
        .models
        .iter()
        .filter(|model| model.provider_id == provider.id)
        .filter(|model| editing_id != Some(model.id.as_str()))
        .collect::<Vec<_>>();
    let provider_id = provider.id.clone();
    let mut models = div().w_full().flex().flex_col().gap_1();

    if let Some(editor) = editor {
        models = models.child(model_form(editor, cx));
        if !configured_models.is_empty() {
            models = models.child(setting_divider(cx));
        }
    }

    if configured_models.is_empty() && editor.is_none() {
        models = models.child(
            div()
                .w_full()
                .px_4()
                .py_6()
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
                        .text_color(cx.theme().muted_foreground)
                        .child("Add a remote model ID to use this provider in conversations."),
                ),
        );
    }

    for model in configured_models {
        models = models.child(model_row(model, cx));
    }

    let actions = editor.is_none().then(|| {
        primary_icon_action("add-model", AppIcon::Plus, "Add model", cx)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.begin_add_model(provider_id.clone(), window, cx)
            }))
            .into_any_element()
    });

    section_with_actions(
        "Models",
        Some("Models configured for this provider."),
        actions,
        models,
        cx,
    )
}

fn model_row(model: &Model, cx: &mut Context<OneChat>) -> AnyElement {
    let edit_id = model.id.clone();
    let delete_id = model.id.clone();
    div()
        .w_full()
        .min_h(px(64.0))
        .rounded_lg()
        .bg(cx.theme().transparent)
        .hover(|style| style.bg(cx.theme().list_hover))
        .px_3()
        .py_2()
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
                        .text_sm()
                        .child(model.display_name.clone()),
                )
                .child(
                    div()
                        .pt_1()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(format!(
                            "{} · {}",
                            model.remote_id,
                            model_capability_summary(model)
                        )),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .gap_1()
                .child(
                    icon_action(
                        SharedString::from(format!("edit-model-{}", model.id)),
                        AppIcon::Pencil,
                        IconTone::Muted,
                        "Edit model",
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.begin_edit_model(edit_id.clone(), window, cx)
                    })),
                )
                .child(
                    icon_action(
                        SharedString::from(format!("delete-model-{}", model.id)),
                        AppIcon::Trash,
                        IconTone::Danger,
                        "Delete model",
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.request_delete_model(delete_id.clone(), window, cx)
                    })),
                ),
        )
        .into_any_element()
}

pub(super) fn model_capability_summary(model: &Model) -> String {
    let capabilities = &model.capabilities;
    let mut labels = Vec::new();
    if capabilities.vision {
        labels.push("Vision");
    }
    if capabilities.audio {
        labels.push("Audio");
    }
    if capabilities.tools {
        labels.push("Tools");
    }
    if model.reasoning.is_some() {
        labels.push("Reasoning");
    }
    if labels.is_empty() {
        "No core capabilities".into()
    } else {
        labels.join(", ")
    }
}
