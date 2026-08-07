mod editors;
mod forms;
mod navigation;
mod pages;
mod providers;

pub(crate) use editors::{
    Capability, ModelEditor, ModelFetchStatus, ProviderEditor, SettingsSection,
};
use forms::{model_form, provider_form};
use navigation::settings_sidebar;
use pages::{default_models_page, general_page, system_prompts_page};
#[cfg(test)]
use providers::model_capability_summary;
use providers::{new_provider_page, provider_page};

use std::collections::BTreeMap;

use gpui::{
    AnyElement, App, Bounds, Context, Div, ElementId, Entity, FontWeight, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, SharedString, Stateful, canvas, deferred,
    div, prelude::*, px, rgba,
};

use super::{
    components::{
        IconTone, UiIcon, button, compact_button, large_svg_icon_button, primary_button,
        primary_svg_icon_button, svg_icon, svg_icon_button,
    },
    composer::{Composer, PickerDirection},
    theme::Colors,
};
use crate::{
    desktop::app::{ConnectionTestStatus, OneChat},
    domain::{
        MAX_MESSAGE_WIDTH_RATIO, MIN_MESSAGE_WIDTH_RATIO, Model, ModelCapabilities, Provider,
        ProviderKind, Theme, now_timestamp,
    },
    providers::AvailableModel,
};

pub(crate) fn render(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let detail = match &app.settings_ui.section {
        SettingsSection::General => general_page(app, colors, scale_factor, cx),
        SettingsSection::DefaultModels => default_models_page(app, colors, scale_factor, cx),
        SettingsSection::SystemPrompts => system_prompts_page(app, colors, scale_factor, cx),
        SettingsSection::Provider(provider_id) => app
            .data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == *provider_id)
            .map(|provider| provider_page(app, provider, colors, scale_factor, cx))
            .unwrap_or_else(|| general_page(app, colors, scale_factor, cx)),
        SettingsSection::NewProvider => new_provider_page(app, colors, scale_factor, cx),
    };

    div()
        .id("settings-page")
        .size_full()
        .min_w_0()
        .flex()
        .child(settings_sidebar(app, colors, scale_factor, cx))
        .child(detail)
        .into_any_element()
}

fn detail_page(content: impl IntoElement) -> AnyElement {
    div()
        .id("settings-detail-scroll")
        .min_w_0()
        .flex_1()
        .h_full()
        .overflow_y_scroll()
        .px_8()
        .py_7()
        .child(div().mx_auto().w_full().max_w(px(780.0)).child(content))
        .into_any_element()
}

fn page_header(title: &'static str, detail: &'static str, colors: Colors) -> AnyElement {
    div()
        .pb_1()
        .child(
            div()
                .text_size(px(28.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .pt_1()
                .text_sm()
                .text_color(colors.muted)
                .child(detail),
        )
        .into_any_element()
}

fn section(
    title: &'static str,
    detail: Option<&'static str>,
    content: impl IntoElement,
    colors: Colors,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .px_1()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .children(detail.map(|detail| {
                    div()
                        .pt_1()
                        .text_size(px(12.0))
                        .text_color(colors.muted)
                        .child(detail)
                })),
        )
        .child(
            div()
                .rounded_xl()
                .border_1()
                .border_color(colors.border)
                .bg(colors.panel)
                .p_4()
                .child(content),
        )
        .into_any_element()
}

fn setting_row(title: &str, detail: &str, control: impl IntoElement, colors: Colors) -> AnyElement {
    div()
        .rounded_lg()
        .bg(colors.raised)
        .px_4()
        .py_3()
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
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .pt_1()
                        .text_size(px(12.0))
                        .text_color(colors.muted)
                        .child(detail.to_string()),
                ),
        )
        .child(control)
        .into_any_element()
}

fn summary_row(title: &str, value: impl Into<SharedString>, colors: Colors) -> AnyElement {
    div()
        .rounded_lg()
        .bg(colors.raised)
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
                .child(title.to_string()),
        )
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_sm()
                .text_color(colors.muted)
                .child(value.into()),
        )
        .into_any_element()
}

fn field(label: &str, input: Entity<Composer>, colors: Colors) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.muted)
                .child(label.to_string()),
        )
        .child(input)
        .into_any_element()
}

fn error_banner(error: &str, colors: Colors) -> AnyElement {
    div()
        .rounded_xl()
        .border_1()
        .border_color(colors.danger)
        .bg(colors.raised)
        .p_4()
        .text_sm()
        .text_color(colors.danger)
        .child(error.to_string())
        .into_any_element()
}

fn prompt_preview(prompt: &str) -> String {
    const MAX_CHARACTERS: usize = 420;
    let prompt = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = prompt.chars();
    let preview = characters.by_ref().take(MAX_CHARACTERS).collect::<String>();
    if characters.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}

fn parse_headers(value: &str) -> Result<BTreeMap<String, String>, String> {
    if value.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    serde_json::from_str(value).map_err(|error| format!("Invalid headers JSON: {error}"))
}

fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_must_be_a_string_map() {
        assert_eq!(
            parse_headers(r#"{"X-Test":"value"}"#).unwrap(),
            BTreeMap::from([("X-Test".into(), "value".into())])
        );
        assert!(parse_headers(r#"{"X-Test":1}"#).is_err());
        assert!(parse_headers("[]").is_err());
    }

    #[test]
    fn model_summary_stays_scannable() {
        let capabilities = ModelCapabilities {
            vision: true,
            thinking: true,
            ..ModelCapabilities::default()
        };
        assert_eq!(
            model_capability_summary(&capabilities),
            "Streaming, Vision, Thinking"
        );
    }
}
