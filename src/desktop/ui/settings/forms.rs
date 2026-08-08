use super::*;

pub(super) fn provider_kind_select(editor: &ProviderEditor) -> AnyElement {
    Select::new(&editor.kind)
        .large()
        .h(px(40.0))
        .px(px(12.0))
        .rounded(px(10.0))
        .placeholder("Provider type")
        .w_full()
        .into_any_element()
}

fn form_input(state: &Entity<InputState>, label: &'static str) -> Input {
    Input::new(state).large().max_h(px(40.0)).aria_label(label)
}

pub(super) fn provider_form(editor: &ProviderEditor, cx: &mut Context<OneChat>) -> AnyElement {
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

pub(super) fn model_form(editor: &ModelEditor, cx: &mut Context<OneChat>) -> AnyElement {
    let title = if editor.is_new() {
        "Add Model"
    } else {
        "Edit Model"
    };
    let model_id_detail = match &editor.fetch_status {
        ModelFetchStatus::Loaded if !editor.available_models.is_empty() => format!(
            "Search discovered models or type a custom ID · {} available",
            editor.available_models.len()
        ),
        _ => "Search discovered models or type a custom ID".into(),
    };
    let actions = div()
        .flex_none()
        .flex()
        .items_center()
        .gap_2()
        .child(
            icon_action(
                "cancel-model",
                AppIcon::Close,
                IconTone::Muted,
                "Cancel",
                cx,
            )
            .on_click(cx.listener(|this, _, _, cx| this.cancel_model_editor(cx))),
        )
        .child(
            primary_icon_action("save-model", AppIcon::Save, "Save model", cx)
                .on_click(cx.listener(|this, _, _, cx| this.save_model(cx))),
        );

    div()
        .w_full()
        .p_2()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(actions),
        )
        .child(
            Form::vertical()
                .columns(2)
                .child(
                    Field::new()
                        .label("Model ID")
                        .required(true)
                        .description(model_id_detail)
                        .col_span(2)
                        .child(
                            Combobox::new(&editor.remote_id)
                                .large()
                                .h(px(40.0))
                                .px(px(12.0))
                                .rounded(px(10.0))
                                .placeholder("Enter or select a model ID…")
                                .search_placeholder("Search or enter a model ID…")
                                .menu_max_h(px(260.0))
                                .empty(|_, cx| {
                                    div()
                                        .p_3()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Type a model ID to use it directly")
                                }),
                        ),
                )
                .children(model_fetch_status(editor, cx).map(|field| field.col_span(2)))
                .child(
                    Field::new()
                        .label("Display Name")
                        .child(form_input(&editor.display_name, "Display name")),
                )
                .child(
                    Field::new()
                        .label("Core Capabilities")
                        .child(capability_group(&Capability::CORE, editor, cx)),
                ),
        )
        .into_any_element()
}

fn model_fetch_status(editor: &ModelEditor, cx: &mut Context<OneChat>) -> Option<Field> {
    let content = match &editor.fetch_status {
        ModelFetchStatus::Loading => div()
            .flex()
            .items_center()
            .gap_2()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(Spinner::new().small())
            .child("Loading available models…")
            .into_any_element(),
        ModelFetchStatus::Failed(error) => div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Alert::error("model-fetch-error", error.clone()).small())
            .child(
                icon_action(
                    "retry-model-list",
                    AppIcon::Regenerate,
                    IconTone::Muted,
                    "Retry loading models",
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.retry_available_models(cx))),
            )
            .into_any_element(),
        ModelFetchStatus::Loaded if editor.available_models.is_empty() => Alert::info(
            "model-fetch-empty",
            "No unconfigured models were returned. You can enter an ID manually.",
        )
        .small()
        .into_any_element(),
        ModelFetchStatus::Loaded => return None,
    };
    Some(Field::new().label_indent(false).child(content))
}

fn capability_group(
    capabilities: &'static [Capability],
    editor: &ModelEditor,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .min_h(px(32.0))
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .children(capabilities.iter().map(|capability| {
            let capability = *capability;
            let enabled = editor.capability(capability);
            Button::new(SharedString::from(format!("capability-{capability:?}")))
                .large()
                .compact()
                .h(px(40.0))
                .px(px(12.0))
                .rounded(px(10.0))
                .label(capability.label())
                .selected(enabled)
                .toggled(enabled)
                .when(enabled, |button| {
                    button
                        .border_color(cx.theme().primary.opacity(0.35))
                        .text_color(cx.theme().primary)
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_model_capability(capability, !enabled, cx)
                }))
        }))
        .into_any_element()
}
