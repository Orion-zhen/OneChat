mod editor;

pub use editor::GenerationConfigEditor;

use std::{fmt::Display, str::FromStr, time::Duration};

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Context, Entity, FontWeight, div,
    ease_out_quint, prelude::*, px,
};
use serde_json::{Map, Value};

use super::{
    components::{button, icon_button, primary_button},
    composer::Composer,
    theme::Colors,
};
use crate::{
    desktop::app::OneChat,
    domain::{Conversation, GenerationConfig, Model, RequestStatus, SystemPromptSource},
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
    overlay: bool,
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
        InspectorTab::Model => render_model(app, colors, cx),
        InspectorTab::Context => render_context(app, colors, cx),
        InspectorTab::Info => render_info(app, colors),
    };

    let inspector = div()
        .w(px(340.0))
        .h_full()
        .flex_none()
        .when(overlay, |element| {
            element.absolute().top_0().right_0().bottom_0().shadow_lg()
        })
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
        );
    inspector
        .with_animation(
            if overlay {
                "inspector-overlay-in"
            } else {
                "inspector-docked-in"
            },
            Animation::new(Duration::from_millis(200)).with_easing(ease_out_quint()),
            |inspector, delta| {
                inspector
                    .opacity(0.78 + delta * 0.22)
                    .w(px(300.0 + 40.0 * delta))
            },
        )
        .into_any_element()
}

fn render_model(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
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
        .child(inspector_field("Model", &model.display_name, colors))
        .child(inspector_field("Provider", provider, colors))
        .child(inspector_field(
            "Capabilities",
            &capability_summary(model),
            colors,
        ))
        .child(
            button("inspector-choose-model", "Change model", colors)
                .on_click(cx.listener(|this, _, _, cx| this.open_model_picker(cx))),
        );

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
    if capabilities.temperature {
        parameters = parameters.child(parameter_field(
            "Temperature",
            editor.temperature.clone(),
            colors,
        ));
    }
    if capabilities.top_p {
        parameters = parameters.child(parameter_field("Top P", editor.top_p.clone(), colors));
    }
    if capabilities.top_k {
        parameters = parameters.child(parameter_field("Top K", editor.top_k.clone(), colors));
    }
    if capabilities.max_output_tokens {
        parameters = parameters.child(parameter_field(
            "Max Output",
            editor.max_output_tokens.clone(),
            colors,
        ));
    }
    if capabilities.frequency_penalty {
        parameters = parameters.child(parameter_field(
            "Frequency Penalty",
            editor.frequency_penalty.clone(),
            colors,
        ));
    }
    if capabilities.presence_penalty {
        parameters = parameters.child(parameter_field(
            "Presence Penalty",
            editor.presence_penalty.clone(),
            colors,
        ));
    }
    if capabilities.seed {
        parameters = parameters.child(parameter_field("Seed", editor.seed.clone(), colors));
    }
    if capabilities.stop_sequences {
        parameters = parameters.child(parameter_field(
            "Stop Sequences",
            editor.stop_sequences.clone(),
            colors,
        ));
    }
    if capabilities.thinking_budget {
        parameters = parameters.child(parameter_field(
            "Thinking Budget",
            editor.thinking_budget.clone(),
            colors,
        ));
    }

    parameters
        .child(parameter_field(
            "Provider-specific Parameters (JSON object)",
            editor.extra.clone(),
            colors,
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
        .child(
            primary_button("save-generation-config", "Save Parameters", colors)
                .on_click(cx.listener(|this, _, _, cx| this.save_generation_config(cx))),
        )
        .into_any_element()
}

fn render_context(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let Some(conversation) = app.current_conversation() else {
        return notice("Select a conversation to inspect its context.", colors);
    };
    let prompt = if conversation.system_prompt.content.trim().is_empty() {
        "None".to_string()
    } else {
        conversation.system_prompt.content.clone()
    };
    let source = match conversation.system_prompt.source {
        SystemPromptSource::None => "None",
        SystemPromptSource::FromDefault => "From default snapshot",
        SystemPromptSource::Custom => "Custom",
    };
    let estimated_tokens = estimate_context_tokens(app);

    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(inspector_field("System Prompt", &prompt, colors))
        .child(inspector_field("Prompt source", source, colors))
        .child(inspector_field(
            "Messages",
            &app.current_messages().len().to_string(),
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
        .map(|conversation| conversation.system_prompt.content.chars().count())
        .unwrap_or_default()
        + app
            .current_messages()
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

fn parameter_field(label: &str, input: Entity<Composer>, colors: Colors) -> AnyElement {
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
