mod editors;
mod forms;
mod navigation;
mod pages;
mod providers;

pub(crate) use editors::{
    Capability, DefaultModelItem, FontFamilyItem, McpServerEditor, McpServerEditorMode,
    McpServerTransportEditor, ModelEditor, ModelFetchStatus, ModelIdDelegate, PromptPresetEditor,
    PromptSelectItem, ProviderEditor, ProviderKindItem, SearchableItems, SettingsSection,
    font_family_label,
};
use forms::{mcp_server_form, model_form, provider_form};
use navigation::settings_sidebar;
use pages::{
    default_models_page, general_page, mcp_page, prompt_preset_dialog_body, system_prompts_page,
};
use providers::{new_provider_page, provider_page};

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use gpui::{
    AlignItems, AnyElement, App, Context, Div, ElementId, Entity, FontWeight, SharedString,
    Stateful, Task, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, IndexPath, Selectable as _, Sizable as _,
    WindowExt as _,
    alert::Alert,
    button::{Button, ButtonVariants as _},
    combobox::{Combobox, ComboboxState},
    dialog::Dialog,
    form::{Field, Form},
    input::{Input, InputContentType, InputState},
    searchable_list::{SearchableListDelegate, SearchableListItem},
    select::{Select, SelectState},
    slider::{Slider, SliderState},
    spinner::Spinner,
    switch::Switch,
    tab::{Tab, TabBar},
};

use super::{
    SIDEBAR_WIDTH,
    icons::{AppIcon, IconTone, render_icon},
};
use crate::{
    desktop::app::{ConnectionTestStatus, DefaultModelRole, FontRole, OneChat, Page},
    domain::{
        DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT, Model, ModelCapabilities, Provider, ProviderKind,
        SendMessageShortcut, SystemPromptPreset, Theme, now_timestamp,
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

pub(crate) fn sync_controls(app: &mut OneChat, window: &mut Window, cx: &mut Context<OneChat>) {
    let message_font_size = app.settings().message_font_size();
    if (app
        .settings_ui
        .message_font_size_slider
        .read(cx)
        .value()
        .start()
        - message_font_size)
        .abs()
        > f32::EPSILON
    {
        app.settings_ui
            .message_font_size_slider
            .update(cx, |slider, cx| {
                slider.set_value(message_font_size, window, cx)
            });
    }

    let opacity = app.settings().background_opacity();
    if (app
        .settings_ui
        .background_opacity_slider
        .read(cx)
        .value()
        .start()
        - opacity)
        .abs()
        > f32::EPSILON
    {
        app.settings_ui
            .background_opacity_slider
            .update(cx, |slider, cx| slider.set_value(opacity, window, cx));
    }

    let ratio = app.settings().message_width_ratio();
    if (app
        .settings_ui
        .message_width_slider
        .read(cx)
        .value()
        .start()
        - ratio)
        .abs()
        > f32::EPSILON
    {
        app.settings_ui
            .message_width_slider
            .update(cx, |slider, cx| slider.set_value(ratio, window, cx));
    }

    let primary_items = default_model_items(app, DefaultModelRole::Primary);
    let primary_changed = primary_items != app.settings_ui.synced_primary_models;
    if primary_changed {
        app.settings_ui
            .synced_primary_models
            .clone_from(&primary_items);
        app.settings_ui
            .primary_model_select
            .update(cx, |select, cx| select.set_items(primary_items, window, cx));
    }
    let primary_value = app.settings().primary_model_id.clone().map(Some);
    if primary_changed
        || app
            .settings_ui
            .primary_model_select
            .read(cx)
            .selected_value()
            .cloned()
            != primary_value
    {
        app.settings_ui
            .primary_model_select
            .update(cx, |select, cx| match primary_value.as_ref() {
                Some(value) => select.set_selected_value(value, window, cx),
                None => select.set_selected_index(None, window, cx),
            });
    }

    let title_items = default_model_items(app, DefaultModelRole::TitleGeneration);
    let title_changed = title_items != app.settings_ui.synced_title_models;
    if title_changed {
        app.settings_ui.synced_title_models.clone_from(&title_items);
        app.settings_ui
            .title_model_select
            .update(cx, |select, cx| select.set_items(title_items, window, cx));
    }
    let title_value = Some(app.settings().title_generation_model_id.clone());
    if title_changed
        || app
            .settings_ui
            .title_model_select
            .read(cx)
            .selected_value()
            .cloned()
            != title_value
    {
        app.settings_ui.title_model_select.update(cx, |select, cx| {
            select.set_selected_value(
                &app.settings().title_generation_model_id.clone(),
                window,
                cx,
            )
        });
    }

    let prompt_items = default_prompt_items(app);
    let prompts_changed = prompt_items != app.settings_ui.synced_prompts;
    if prompts_changed {
        app.settings_ui.synced_prompts.clone_from(&prompt_items);
        app.settings_ui
            .default_prompt_select
            .update(cx, |select, cx| select.set_items(prompt_items, window, cx));
    }
    let prompt_value = Some(app.settings().default_system_prompt_preset.clone());
    if prompts_changed
        || app
            .settings_ui
            .default_prompt_select
            .read(cx)
            .selected_value()
            .cloned()
            != prompt_value
    {
        app.settings_ui
            .default_prompt_select
            .update(cx, |select, cx| {
                select.set_selected_value(
                    &app.settings().default_system_prompt_preset.clone(),
                    window,
                    cx,
                )
            });
    }

    if let Some(editor) = &mut app.settings_ui.model_editor {
        editor.sync_combobox(window, cx);
    }
}

fn default_model_items(app: &OneChat, role: DefaultModelRole) -> Vec<DefaultModelItem> {
    let selected_id = match role {
        DefaultModelRole::Primary => app.settings().primary_model_id.as_deref(),
        DefaultModelRole::TitleGeneration => app.settings().title_generation_model_id.as_deref(),
    };
    let mut items = Vec::new();
    if role == DefaultModelRole::TitleGeneration {
        items.push(DefaultModelItem::new(
            None,
            "Use Primary Model",
            "Follow the primary model setting",
            false,
        ));
    }
    for model in &app.data.snapshot.models {
        let availability = app.model_availability(model);
        if availability.is_err() && selected_id != Some(model.id.as_str()) {
            continue;
        }
        let provider = app
            .provider_for_model(model)
            .map(|provider| provider.name.as_str())
            .unwrap_or("Missing provider");
        let detail = availability.map_or_else(
            |reason| format!("Unavailable · {reason}"),
            |_| format!("{} · {provider}", model.remote_id),
        );
        items.push(DefaultModelItem::new(
            Some(model.id.clone()),
            model.display_name.clone(),
            detail,
            availability.is_err(),
        ));
    }
    if let Some(selected_id) = selected_id
        && !items
            .iter()
            .any(|item| item.value() == &Some(selected_id.to_string()))
    {
        items.push(DefaultModelItem::new(
            Some(selected_id.to_string()),
            format!("Missing · {selected_id}"),
            "The configured model no longer exists",
            true,
        ));
    }
    items
}

fn default_prompt_items(app: &OneChat) -> Vec<PromptSelectItem> {
    let selected = app.settings().default_system_prompt_preset.as_deref();
    let mut items = vec![PromptSelectItem::new(None, "No System Prompt", false)];
    items.extend(app.data.snapshot.prompt_presets.iter().map(|preset| {
        PromptSelectItem::new(Some(preset.name.clone()), preset.name.clone(), false)
    }));
    if let Some(selected) = selected
        && !app
            .data
            .snapshot
            .prompt_presets
            .iter()
            .any(|preset| preset.name == selected)
    {
        items.push(PromptSelectItem::new(
            Some(selected.to_string()),
            format!("Missing · {selected}"),
            true,
        ));
    }
    items
}

pub(crate) fn prompt_preset_dialog(
    dialog: Dialog,
    app: Entity<OneChat>,
    _window: &mut Window,
    cx: &mut App,
) -> Dialog {
    let state = app.read(cx);
    let editing = state.settings_ui.prompt_preset_editor.is_some();
    let title =
        state
            .settings_ui
            .prompt_preset_editor
            .as_ref()
            .map_or("View prompt preset", |editor| {
                if editor.original_name().is_some() {
                    "Edit prompt preset"
                } else {
                    "New prompt preset"
                }
            });
    let body = prompt_preset_dialog_body(state, cx);

    let cancel_app = app.clone();
    let close_app = app.clone();
    let header = div()
        .relative()
        .w_full()
        .h(px(52.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            icon_action(
                "close-prompt-preset",
                AppIcon::Close,
                IconTone::Muted,
                "Close",
                cx,
            )
            .absolute()
            .left(px(12.0))
            .top(px(11.0))
            .size(px(30.0))
            .rounded(px(9.0))
            .on_click(move |_, window, cx| {
                close_app.update(cx, |app, cx| {
                    if editing {
                        app.cancel_prompt_preset_edit(cx);
                    } else {
                        app.settings_ui.viewed_prompt_preset = None;
                        app.settings_ui.form_error = None;
                        cx.notify();
                    }
                });
                window.close_dialog(cx);
            }),
        )
        .child(
            div()
                .px(px(52.0))
                .text_size(px(15.0))
                .line_height(px(20.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .when(editing, |header| {
            let save_app = app.clone();
            header.child(
                primary_icon_action("save-prompt-preset", AppIcon::Save, "Save prompt", cx)
                    .absolute()
                    .right(px(12.0))
                    .top(px(11.0))
                    .size(px(30.0))
                    .rounded(px(9.0))
                    .on_click(move |_, window, cx| {
                        let saved = save_app.update(cx, |app, cx| app.save_prompt_preset(cx));
                        if saved {
                            window.close_dialog(cx);
                        }
                    }),
            )
        });

    let mut dialog = dialog
        .width(px(560.0))
        .margin_top(px(56.0))
        .p_0()
        .rounded(px(18.0))
        .bg(cx.theme().popover)
        .close_button(false)
        .title(header)
        .child(body)
        .on_cancel(move |_, _, cx| {
            cancel_app.update(cx, |app, cx| {
                if editing {
                    app.cancel_prompt_preset_edit(cx);
                } else {
                    app.settings_ui.viewed_prompt_preset = None;
                    app.settings_ui.form_error = None;
                    cx.notify();
                }
            });
            true
        });

    if editing {
        let save_app = app;
        dialog =
            dialog.on_ok(move |_, _, cx| save_app.update(cx, |app, cx| app.save_prompt_preset(cx)));
    }
    dialog
}

pub(crate) fn render(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
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
        .child(settings_sidebar(app, cx))
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
