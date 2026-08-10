mod controls;
mod dialog;
mod editors;
mod forms;
mod navigation;
mod pages;
mod providers;
mod reasoning;
mod theme_color;

pub(crate) use controls::sync_controls;
pub(crate) use dialog::{prompt_preset_dialog, prompt_variable_dialog};
pub(crate) use editors::{
    Capability, DefaultModelItem, FontFamilyItem, McpServerEditor, McpServerEditorMode,
    McpServerTransportEditor, ModelEditor, ModelFetchStatus, ModelIdDelegate, PromptPresetEditor,
    PromptSelectItem, PromptVariableEditor, PromptVariableKind, PromptVariableTestStatus,
    ProviderEditor, ProviderKindItem, ReasoningPresetSelectItem, SearchableItems, SettingsSection,
    font_family_label,
};
use forms::{mcp_server_form, model_form, provider_form, provider_form_actions};
use navigation::settings_sidebar;
use pages::{
    default_models_page, general_page, mcp_page, prompt_preset_dialog_body,
    prompt_variable_dialog_body, system_prompts_page,
};
use providers::{new_provider_page, provider_page};
pub(crate) use reasoning::{
    KnownReasoningFormatItem, ModelReasoningEditor, ReasoningEditorMode, ReasoningParameterEditor,
    ReasoningParameterPathEditor, ReasoningParameterScope, ReasoningParameterType,
    default_reasoning_format,
};
pub(crate) use theme_color::ThemeColorControl;
use theme_color::theme_color_picker;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use gpui::{
    AlignItems, AnyElement, App, Context, Div, DragMoveEvent, ElementId, Entity, FontWeight,
    Render, SharedString, Stateful, Task, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, IndexPath, Selectable as _, Sizable as _,
    WindowExt as _,
    alert::Alert,
    button::{Button, ButtonVariants as _},
    combobox::{Combobox, ComboboxEvent, ComboboxState},
    dialog::Dialog,
    form::{Field, Form},
    input::{Escape as InputEscape, Input, InputContentType, InputState},
    searchable_list::{SearchableListDelegate, SearchableListItem},
    select::{Select, SelectEvent, SelectState},
    slider::{Slider, SliderState},
    spinner::Spinner,
    switch::Switch,
    tab::{Tab, TabBar},
};

use super::{
    copy_button::CopyButton,
    icons::{AppIcon, IconTone, render_icon},
};
use crate::{
    desktop::app::{ConnectionTestStatus, DefaultModelRole, FontRole, OneChat},
    domain::{
        CustomReasoningPreset, DEFAULT_PROMPT_COMMAND_TIMEOUT_MS,
        DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT, KnownReasoningFormat, KnownReasoningPreset, Model,
        ModelCapabilities, ModelReasoningConfig, PROVIDER_DEFAULT_REASONING_PRESET,
        PromptVariableSource, Provider, ProviderKind, ReasoningLevel, ReasoningParameter,
        ReasoningParameterValue, SendMessageShortcut, SystemPromptPreset, Theme, now_timestamp,
        prompt_variable_name_is_valid,
    },
    mcp::{
        McpConfig, McpHttpServerConfig, McpOAuthConfig, McpOAuthFlow, McpServerConfig,
        McpServerSnapshot, McpServerStatus, McpServerTransportSnapshot, McpStdioServerConfig,
    },
    providers::AvailableModel,
};

fn icon_action(
    id: impl Into<ElementId>,
    icon: AppIcon,
    tone: IconTone,
    tooltip: &'static str,
    cx: &App,
) -> Button {
    Button::new(id)
        .ghost()
        .tooltip(tooltip)
        .size(px(30.0))
        .p_0()
        .child(render_icon(icon, tone, 16.0, cx))
}

fn primary_icon_action(
    id: impl Into<ElementId>,
    icon: AppIcon,
    tooltip: &'static str,
    cx: &App,
) -> Button {
    icon_action(id, icon, IconTone::OnAccent, tooltip, cx).primary()
}

fn danger_icon_action(
    id: impl Into<ElementId>,
    icon: AppIcon,
    tooltip: &'static str,
    cx: &App,
) -> Button {
    icon_action(id, icon, IconTone::OnAccent, tooltip, cx).danger()
}

pub(crate) fn render(app: &OneChat, sidebar_width: f32, cx: &mut Context<OneChat>) -> AnyElement {
    let detail = match &app.settings_ui.section {
        SettingsSection::General => general_page(app, cx),
        SettingsSection::DefaultModels => default_models_page(app, cx),
        SettingsSection::SystemPrompts => system_prompts_page(app, cx),
        SettingsSection::Mcp => mcp_page(app, cx),
        SettingsSection::Provider(provider_id) => app
            .data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == *provider_id)
            .map(|provider| provider_page(app, provider, cx))
            .unwrap_or_else(|| general_page(app, cx)),
        SettingsSection::NewProvider => new_provider_page(app, cx),
    };

    div()
        .id("settings-page")
        .size_full()
        .min_w_0()
        .flex()
        .child(settings_sidebar(app, sidebar_width, cx))
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
        .py_8()
        .child(div().mx_auto().w_full().max_w(px(760.0)).child(content))
        .into_any_element()
}

fn page_header(title: &'static str, detail: &'static str, cx: &App) -> AnyElement {
    div()
        .pb_1()
        .child(
            div()
                .text_size(px(28.0))
                .line_height(px(34.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .pt_1()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(detail),
        )
        .into_any_element()
}

fn section(
    title: &'static str,
    detail: Option<&'static str>,
    content: impl IntoElement,
    cx: &App,
) -> AnyElement {
    section_with_actions(title, detail, None, content, cx)
}

fn section_with_actions(
    title: &'static str,
    detail: Option<&'static str>,
    actions: Option<AnyElement>,
    content: impl IntoElement,
    cx: &App,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .px_1()
                .flex()
                .items_end()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .min_w_0()
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
                                .line_height(px(18.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(detail)
                        })),
                )
                .children(actions),
        )
        .child(
            div()
                .w_full()
                .rounded_xl()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().popover)
                .p_2()
                .child(content),
        )
        .into_any_element()
}

fn setting_row(title: &str, detail: &str, control: impl IntoElement, cx: &App) -> AnyElement {
    setting_row_content(title, Some(detail), None, control, cx)
}

fn setting_row_with_preview(
    title: &str,
    preview: impl IntoElement,
    control: impl IntoElement,
    cx: &App,
) -> AnyElement {
    setting_row_content(title, None, Some(preview.into_any_element()), control, cx)
}

fn setting_row_content(
    title: &str,
    detail: Option<&str>,
    preview: Option<AnyElement>,
    control: impl IntoElement,
    cx: &App,
) -> AnyElement {
    let label = div()
        .min_w_0()
        .flex_1()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_string()),
        )
        .children(detail.map(|detail| {
            div()
                .pt_1()
                .text_size(px(12.0))
                .line_height(px(18.0))
                .text_color(cx.theme().muted_foreground)
                .child(detail.to_string())
        }))
        .children(preview);

    div()
        .w_full()
        .min_h(px(68.0))
        .rounded(px(10.0))
        .bg(cx.theme().transparent)
        .px_4()
        .py_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_5()
        .child(label)
        .child(div().flex_none().child(control))
        .into_any_element()
}

fn setting_divider(cx: &App) -> AnyElement {
    div()
        .mx_4()
        .h(px(1.0))
        .bg(cx.theme().border)
        .into_any_element()
}

fn summary_row(title: &str, value: impl Into<SharedString>, cx: &App) -> AnyElement {
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
                .child(title.to_string()),
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
                .child(value.into()),
        )
        .into_any_element()
}

fn error_banner(error: &str) -> AnyElement {
    Alert::error("settings-form-error", error.to_string()).into_any_element()
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

fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn status_pill(label: impl Into<SharedString>, accent: bool, cx: &App) -> AnyElement {
    div()
        .flex_none()
        .rounded_full()
        .bg(if accent {
            cx.theme().accent
        } else {
            cx.theme().muted
        })
        .px_2()
        .py_1()
        .text_size(px(10.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if accent {
            cx.theme().primary
        } else {
            cx.theme().muted_foreground
        })
        .child(label.into())
        .into_any_element()
}

fn stretching_column() -> Div {
    let mut column = div().flex().flex_col();
    column.style().align_items = Some(AlignItems::Stretch);
    column
}
