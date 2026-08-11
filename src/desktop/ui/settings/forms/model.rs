use super::*;

mod custom_presets;
mod parameters;
mod reasoning;

use custom_presets::*;
use parameters::*;
use reasoning::*;

pub(in crate::desktop::ui::settings) fn model_form(
    editor: &ModelEditor,
    cx: &mut Context<OneChat>,
) -> AnyElement {
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
            Compact
                .icon_action(
                    "cancel-model",
                    AppIcon::Close,
                    IconTone::Muted,
                    "Cancel",
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.cancel_model_editor(cx))),
        )
        .child(
            Compact
                .primary_icon_action("save-model", AppIcon::Save, "Save model", cx)
                .on_click(cx.listener(|this, _, _, cx| this.save_model(cx))),
        );

    div()
        .w_full()
        .p_2()
        .flex()
        .flex_col()
        .gap_4()
        .child(editor_header(title, actions))
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
                        .label("Context Window")
                        .child(form_input(&editor.context_window, "Unknown or token count")),
                )
                .child(
                    Field::new()
                        .label("Core Capabilities")
                        .col_span(2)
                        .child(capability_group(&Capability::CORE, editor, cx)),
                ),
        )
        .child(model_reasoning_form(&editor.reasoning, cx))
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
                Compact
                    .icon_action(
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
                        .border_color(crate::desktop::ui::theme::palette(cx).accent_border)
                        .text_color(cx.theme().primary)
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_model_capability(capability, !enabled, cx)
                }))
        }))
        .into_any_element()
}
