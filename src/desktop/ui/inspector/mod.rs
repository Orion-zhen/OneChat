mod editor;

pub use editor::{GenerationConfigEditor, GenerationParameter};

use std::{fmt::Display, str::FromStr};

use gpui::{
    AnyElement, App, Context, Entity, FontWeight, SharedString, deferred, div, prelude::*, px,
};
use serde_json::{Map, Value};

use super::{
    components::{
        IconTone, UiIcon, button, icon_button, primary_button, svg_icon, svg_icon_button,
    },
    composer::Composer,
    theme::Colors,
};
use crate::{
    desktop::app::OneChat,
    domain::{Conversation, GenerationConfig, Model, RequestStatus},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InspectorTab {
    #[default]
    Model,
    Context,
    Info,
}

impl InspectorTab {
    fn label(self) -> &'static str {
        match self {
            Self::Model => "Model",
            Self::Context => "Context",
            Self::Info => "Info",
        }
    }
}

pub(crate) fn render(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let mut tabs = div().rounded_lg().bg(colors.raised).p_1().flex().gap_1();
    for tab in [
        InspectorTab::Model,
        InspectorTab::Context,
        InspectorTab::Info,
    ] {
        let id = match tab {
            InspectorTab::Model => "inspector-tab-model",
            InspectorTab::Context => "inspector-tab-context",
            InspectorTab::Info => "inspector-tab-info",
        };
        let selected = app.navigation.inspector_tab == tab;
        tabs = tabs.child(
            div()
                .id(id)
                .flex_1()
                .rounded_md()
                .px_2()
                .py_2()
                .text_center()
                .text_size(px(12.0))
                .font_weight(if selected {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .when(selected, |element| element.bg(colors.hover).shadow_sm())
                .cursor_pointer()
                .hover(move |style| style.bg(colors.hover))
                .active(move |style| style.bg(colors.accent_soft))
                .on_click(cx.listener(move |this, _, _, cx| this.set_inspector_tab(tab, cx)))
                .child(tab.label()),
        );
    }

    let content = match app.navigation.inspector_tab {
        InspectorTab::Model => render_model(app, colors, scale_factor, cx),
        InspectorTab::Context => render_context(app, colors, cx),
        InspectorTab::Info => render_info(app, colors),
    };

    div()
        .absolute()
        .occlude()
        .top_0()
        .right_0()
        .bottom_0()
        .w(px(340.0))
        .shadow_lg()
        .border_l_1()
        .border_color(colors.border)
        .bg(colors.toolbar)
        .p_4()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Details"),
                )
                .child(
                    icon_button("close-inspector", "×", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_inspector(cx))),
                ),
        )
        .child(tabs)
        .child(
            div()
                .id("inspector-content-scroll")
                .min_h_0()
                .flex_1()
                .overflow_y_scroll()
                .child(content),
        )
        .into_any_element()
}

fn render_model(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let Some(conversation) = app.current_conversation() else {
        return notice("Select a conversation to configure its model.", colors);
    };
    let Some(model) = app.current_model() else {
        return div()
            .flex()
            .flex_col()
            .gap_3()
            .child(notice("This conversation has no model.", colors))
            .child(
                button("inspector-choose-model-empty", "Choose model", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.open_model_picker(cx))),
            )
            .into_any_element();
    };

    let provider = app
        .current_provider()
        .map(|provider| provider.name.as_str())
        .unwrap_or("Missing provider");
    let ignored = conversation
        .generation_config
        .filtered_for(&model.capabilities)
        .1;
    let Some(editor) = app.chat.generation_config_editor.as_ref() else {
        return notice("Opening parameter editor…", colors);
    };

    let mut parameters = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(model_summary(model, provider, colors));

    if !ignored.is_empty() {
        parameters = parameters.child(
            div()
                .rounded_lg()
                .bg(colors.accent_soft)
                .p_3()
                .text_sm()
                .text_color(colors.muted)
                .child(format!(
                    "Not sent by this model: {}. The saved values are preserved.",
                    ignored.join(", ")
                )),
        );
    }

    let capabilities = &model.capabilities;
    for parameter in GenerationParameter::ALL {
        if editor.is_active(parameter) && parameter.supported_by(capabilities) {
            parameters = parameters.child(parameter_field(
                parameter,
                editor.input(parameter),
                colors,
                scale_factor,
                cx,
            ));
        }
    }

    parameters
        .child(add_parameter_select(
            editor,
            capabilities,
            colors,
            scale_factor,
            cx,
        ))
        .children(app.chat.parameter_error.as_ref().map(|error| {
            div()
                .rounded_lg()
                .bg(colors.raised)
                .p_3()
                .text_sm()
                .text_color(colors.danger)
                .child(error.clone())
        }))
        .into_any_element()
}

fn render_context(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let Some(conversation) = app.current_conversation() else {
        return notice("Select a conversation to inspect its context.", colors);
    };
    let prompt = if conversation.system_prompt.trim().is_empty() {
        "None".to_string()
    } else {
        conversation.system_prompt.clone()
    };
    let source = app.system_prompt_label(&conversation.system_prompt);
    let estimated_tokens = estimate_context_tokens(app);

    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(inspector_field("System Prompt", &prompt, colors))
        .child(inspector_field("Prompt source", &source, colors))
        .child(inspector_field(
            "Messages",
            &app.current_context_messages().len().to_string(),
            colors,
        ))
        .child(inspector_field(
            "Estimated context tokens",
            &format!("~{estimated_tokens}"),
            colors,
        ))
        .child(
            primary_button("context-edit-system-prompt", "Edit System Prompt", colors)
                .on_click(cx.listener(|this, _, _, cx| this.begin_edit_system_prompt(cx))),
        )
        .child(
            button("clear-conversation-context", "Clear context", colors)
                .text_color(colors.danger)
                .on_click(cx.listener(|this, _, _, cx| this.request_clear_current_context(cx))),
        )
        .into_any_element()
}

fn render_info(app: &OneChat, colors: Colors) -> AnyElement {
    let request = app.inspected_request();
    let model = request
        .and_then(|request| request.model_id.as_deref())
        .and_then(|id| app.data.snapshot.models.iter().find(|model| model.id == id))
        .or_else(|| app.current_model());
    let provider = request
        .and_then(|request| request.provider_id.as_deref())
        .and_then(|id| {
            app.data
                .snapshot
                .providers
                .iter()
                .find(|provider| provider.id == id)
        })
        .or_else(|| app.current_provider());
    let mut content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(inspector_field(
            "Model",
            model
                .map(|model| model.display_name.as_str())
                .unwrap_or("None"),
            colors,
        ))
        .child(inspector_field(
            "Remote ID",
            model.map(|model| model.remote_id.as_str()).unwrap_or("—"),
            colors,
        ))
        .child(inspector_field(
            "Provider",
            provider
                .map(|provider| provider.name.as_str())
                .unwrap_or("None"),
            colors,
        ));

    let Some(request) = request else {
        return content
            .child(notice("No request information yet.", colors))
            .into_any_element();
    };
    let status = match request.status {
        RequestStatus::Sending => "Sending",
        RequestStatus::Streaming => "Streaming",
        RequestStatus::Stopped => "Stopped",
        RequestStatus::Failed => "Failed",
        RequestStatus::Completed => "Completed",
        RequestStatus::Interrupted => "Interrupted",
    };
    content = content
        .child(inspector_field("Request ID", &request.id, colors))
        .child(inspector_field("Request status", status, colors))
        .child(inspector_field(
            "Input tokens",
            &format_token_count(request.usage.input_tokens, request.usage.estimated),
            colors,
        ))
        .child(inspector_field(
            "Output tokens",
            &format_token_count(request.usage.output_tokens, request.usage.estimated),
            colors,
        ))
        .child(inspector_field(
            "First token",
            &request
                .ttft_ms
                .map_or_else(|| "—".into(), |value| format!("{value} ms")),
            colors,
        ))
        .child(inspector_field(
            "Total time",
            &request
                .duration_ms
                .map_or_else(|| "—".into(), |value| format!("{value} ms")),
            colors,
        ));
    if let Some(error) = &request.error {
        content = content
            .child(inspector_field("Error category", &error.kind, colors))
            .child(
                div()
                    .rounded_lg()
                    .bg(colors.raised)
                    .p_3()
                    .text_sm()
                    .text_color(colors.danger)
                    .child(error.message.clone())
                    .children(error.detail.clone().map(|detail| {
                        div()
                            .pt_2()
                            .text_size(px(11.0))
                            .text_color(colors.muted)
                            .child(detail)
                    })),
            );
    }
    content.into_any_element()
}

pub(crate) fn capability_summary(model: &Model) -> String {
    let capabilities = &model.capabilities;
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
        "No declared capabilities".into()
    } else {
        labels.join(" · ")
    }
}

fn estimate_context_tokens(app: &OneChat) -> usize {
    let characters = app
        .current_conversation()
        .map(|conversation| conversation.system_prompt.chars().count())
        .unwrap_or_default()
        + app
            .current_context_messages()
            .iter()
            .map(|message| message.content.chars().count())
            .sum::<usize>();
    characters.div_ceil(4)
}

fn format_token_count(value: Option<u64>, estimated: bool) -> String {
    value.map_or_else(
        || "—".into(),
        |value| {
            if estimated {
                format!("~{value}")
            } else {
                value.to_string()
            }
        },
    )
}

fn parameter_field(
    parameter: GenerationParameter,
    input: Entity<Composer>,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors.muted)
                        .child(parameter.label()),
                )
                .child(
                    svg_icon_button(
                        SharedString::from(format!("remove-parameter-{}", parameter.id())),
                        UiIcon::Close,
                        IconTone::Muted,
                        colors,
                        scale_factor,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.remove_generation_parameter(parameter, cx)
                    })),
                ),
        )
        .child(input)
        .into_any_element()
}

fn add_parameter_select(
    editor: &GenerationConfigEditor,
    capabilities: &crate::domain::ModelCapabilities,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let available = GenerationParameter::ALL
        .into_iter()
        .filter(|parameter| parameter.supported_by(capabilities) && !editor.is_active(*parameter))
        .collect::<Vec<_>>();
    let disabled = available.is_empty();
    let mut menu = div()
        .id("generation-parameter-options")
        .occlude()
        .absolute()
        .top(px(42.0))
        .left_0()
        .right_0()
        .rounded_lg()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .p_1()
        .flex()
        .flex_col()
        .shadow_lg();
    for parameter in available {
        menu = menu.child(
            div()
                .id(SharedString::from(format!(
                    "generation-parameter-option-{}",
                    parameter.id()
                )))
                .w_full()
                .px_3()
                .py_2()
                .rounded_md()
                .text_sm()
                .cursor_pointer()
                .hover(move |style| style.bg(colors.hover))
                .on_click(
                    cx.listener(move |this, _, _, cx| this.add_generation_parameter(parameter, cx)),
                )
                .child(parameter.label()),
        );
    }

    let trigger = div()
        .id("add-generation-parameter")
        .w_full()
        .px_3()
        .py_2()
        .rounded_lg()
        .bg(colors.raised)
        .flex()
        .items_center()
        .justify_between()
        .text_sm()
        .text_color(if disabled { colors.muted } else { colors.text })
        .when(!disabled, |element| {
            element
                .cursor_pointer()
                .hover(move |style| style.bg(colors.hover))
                .active(move |style| style.bg(colors.accent_soft))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_generation_parameter_menu(cx)))
        })
        .child(if disabled {
            "All available parameters added"
        } else {
            "Add parameter"
        })
        .child(svg_icon(
            if editor.parameter_menu_open {
                UiIcon::ChevronUp
            } else {
                UiIcon::ChevronDown
            },
            IconTone::Muted,
            colors,
            scale_factor,
            14.0,
        ));

    div()
        .relative()
        .w_full()
        .child(trigger)
        .children((editor.parameter_menu_open && !disabled).then(|| deferred(menu).priority(1)))
        .into_any_element()
}

fn model_summary(model: &Model, provider: &str, colors: Colors) -> AnyElement {
    div()
        .rounded_lg()
        .bg(colors.raised)
        .p_3()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(colors.muted)
                .child("Model"),
        )
        .child(
            div()
                .pt_1()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(model.display_name.clone()),
        )
        .child(
            div()
                .pt_1()
                .text_size(px(11.0))
                .text_color(colors.muted)
                .child(format!("{} · {}", provider, capability_summary(model))),
        )
        .into_any_element()
}

fn inspector_field(label: &str, value: &str, colors: Colors) -> AnyElement {
    div()
        .rounded_lg()
        .bg(colors.raised)
        .p_3()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(colors.muted)
                .child(label.to_string()),
        )
        .child(div().pt_1().text_sm().child(value.to_string()))
        .into_any_element()
}

fn notice(message: &str, colors: Colors) -> AnyElement {
    div()
        .rounded_xl()
        .bg(colors.raised)
        .p_4()
        .text_sm()
        .line_height(px(21.0))
        .text_color(colors.muted)
        .child(message.to_string())
        .into_any_element()
}
