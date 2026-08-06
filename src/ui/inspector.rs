use std::{fmt::Display, str::FromStr, time::Duration};

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Context, Entity, FontWeight, div,
    ease_out_quint, prelude::*, px,
};
use serde_json::{Map, Value};

use crate::{
    app::OneChat,
    model::{Conversation, GenerationConfig, Model, RequestStatus, SystemPromptSource},
    ui::{
        composer::Composer,
        shell::{Colors, button},
    },
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

pub struct GenerationConfigEditor {
    conversation_id: String,
    pub temperature: Entity<Composer>,
    pub top_p: Entity<Composer>,
    pub top_k: Entity<Composer>,
    pub max_output_tokens: Entity<Composer>,
    pub frequency_penalty: Entity<Composer>,
    pub presence_penalty: Entity<Composer>,
    pub seed: Entity<Composer>,
    pub stop_sequences: Entity<Composer>,
    pub thinking_budget: Entity<Composer>,
    pub extra: Entity<Composer>,
}

impl GenerationConfigEditor {
    pub fn new(conversation: &Conversation, cx: &mut Context<OneChat>) -> Self {
        let config = &conversation.generation_config;
        Self {
            conversation_id: conversation.id.clone(),
            temperature: optional_input(config.temperature, "Optional number", cx),
            top_p: optional_input(config.top_p, "Optional number", cx),
            top_k: optional_input(config.top_k, "Optional integer", cx),
            max_output_tokens: optional_input(config.max_output_tokens, "Optional integer", cx),
            frequency_penalty: optional_input(config.frequency_penalty, "Optional number", cx),
            presence_penalty: optional_input(config.presence_penalty, "Optional number", cx),
            seed: optional_input(config.seed, "Optional integer", cx),
            stop_sequences: cx.new(|cx| {
                Composer::multiline(
                    config.stop_sequences.join("\n"),
                    "One stop sequence per line",
                    cx,
                )
            }),
            thinking_budget: optional_input(config.thinking_budget, "Optional integer", cx),
            extra: cx.new(|cx| {
                Composer::multiline(
                    serde_json::to_string_pretty(&config.extra).unwrap_or_else(|_| "{}".into()),
                    "Provider-specific JSON object",
                    cx,
                )
            }),
        }
    }

    pub fn is_for(&self, conversation_id: &str) -> bool {
        self.conversation_id == conversation_id
    }

    pub fn build(&self, base: &GenerationConfig, cx: &App) -> Result<GenerationConfig, String> {
        let mut config = base.clone();
        config.temperature = parse_optional_f64("Temperature", self.temperature.read(cx).text())?;
        config.top_p = parse_optional_f64("Top P", self.top_p.read(cx).text())?;
        config.top_k = parse_optional("Top K", self.top_k.read(cx).text())?;
        config.max_output_tokens =
            parse_optional("Max Output", self.max_output_tokens.read(cx).text())?;
        config.frequency_penalty =
            parse_optional_f64("Frequency Penalty", self.frequency_penalty.read(cx).text())?;
        config.presence_penalty =
            parse_optional_f64("Presence Penalty", self.presence_penalty.read(cx).text())?;
        config.seed = parse_optional("Seed", self.seed.read(cx).text())?;
        config.stop_sequences = self
            .stop_sequences
            .read(cx)
            .text()
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect();
        config.thinking_budget =
            parse_optional("Thinking Budget", self.thinking_budget.read(cx).text())?;
        config.extra = parse_json_object(self.extra.read(cx).text())?;
        Ok(config)
    }
}

fn optional_input<T: Display>(
    value: Option<T>,
    placeholder: &'static str,
    cx: &mut Context<OneChat>,
) -> Entity<Composer> {
    cx.new(|cx| {
        Composer::single_line(
            value.map(|value| value.to_string()).unwrap_or_default(),
            placeholder,
            cx,
        )
    })
}

fn parse_optional<T>(label: &str, value: &str) -> Result<Option<T>, String>
where
    T: FromStr,
    T::Err: Display,
{
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    value
        .parse()
        .map(Some)
        .map_err(|error| format!("Invalid {label}: {error}"))
}

fn parse_optional_f64(label: &str, value: &str) -> Result<Option<f64>, String> {
    let value = parse_optional::<f64>(label, value)?;
    if value.is_some_and(|value| !value.is_finite()) {
        return Err(format!("Invalid {label}: value must be finite"));
    }
    Ok(value)
}

pub(crate) fn parse_json_object(value: &str) -> Result<Map<String, Value>, String> {
    if value.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str(value) {
        Ok(Value::Object(object)) => Ok(object),
        Ok(_) => Err("Provider-specific parameters must be a JSON object.".into()),
        Err(error) => Err(format!(
            "Invalid provider-specific parameters JSON: {error}"
        )),
    }
}

pub(crate) fn render(
    app: &OneChat,
    colors: Colors,
    overlay: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let mut tabs = div().flex().gap_2();
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
        tabs = tabs.child(
            button(id, tab.label(), colors)
                .when(app.inspector_tab == tab, |element| {
                    element.bg(colors.accent_soft).border_color(colors.accent)
                })
                .on_click(cx.listener(move |this, _, _, cx| this.set_inspector_tab(tab, cx))),
        );
    }

    let content = match app.inspector_tab {
        InspectorTab::Model => render_model(app, colors, cx),
        InspectorTab::Context => render_context(app, colors, cx),
        InspectorTab::Info => render_info(app, colors),
    };

    let inspector = div()
        .w(px(328.0))
        .h_full()
        .flex_none()
        .when(overlay, |element| {
            element.absolute().top_0().right_0().bottom_0().shadow_lg()
        })
        .border_l_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .p_5()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("Inspector"))
                .child(
                    button("close-inspector", "×", colors)
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
    let reduce_motion = app.settings().reduce_motion;
    inspector
        .with_animation(
            if overlay {
                "inspector-overlay-in"
            } else {
                "inspector-docked-in"
            },
            Animation::new(Duration::from_millis(if reduce_motion { 160 } else { 200 }))
                .with_easing(ease_out_quint()),
            move |inspector, delta| {
                let inspector = inspector.opacity(0.78 + delta * 0.22);
                if reduce_motion {
                    inspector
                } else {
                    inspector.w(px(300.0 + 28.0 * delta))
                }
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
    let Some(editor) = app.generation_config_editor.as_ref() else {
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
                    "已忽略：{}。值仍保留，但请求不会发送。",
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
        .children(app.parameter_error.as_ref().map(|error| {
            div()
                .rounded_lg()
                .bg(colors.raised)
                .p_3()
                .text_sm()
                .text_color(colors.danger)
                .child(error.clone())
        }))
        .child(
            button("save-generation-config", "Save parameters", colors)
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
            button("context-edit-system-prompt", "Edit System Prompt", colors)
                .on_click(cx.listener(|this, _, _, cx| this.begin_edit_system_prompt(cx))),
        )
        .child(
            button("clear-conversation-context", "Clear context", colors)
                .text_color(colors.danger)
                .on_click(cx.listener(|this, _, _, cx| this.clear_current_context(cx))),
        )
        .into_any_element()
}

fn render_info(app: &OneChat, colors: Colors) -> AnyElement {
    let request = app.inspected_request();
    let model = request
        .and_then(|request| request.model_id.as_deref())
        .and_then(|id| app.snapshot.models.iter().find(|model| model.id == id))
        .or_else(|| app.current_model());
    let provider = request
        .and_then(|request| request.provider_id.as_deref())
        .and_then(|id| {
            app.snapshot
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
                            .text_xs()
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
    if capabilities.system_prompt {
        labels.push("System");
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
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.muted)
                .child(label.to_string()),
        )
        .child(input)
        .into_any_element()
}

fn inspector_field(label: &str, value: &str, colors: Colors) -> AnyElement {
    div()
        .border_b_1()
        .border_color(colors.border)
        .pb_3()
        .child(
            div()
                .text_xs()
                .text_color(colors.muted)
                .child(label.to_string()),
        )
        .child(div().pt_1().text_sm().child(value.to_string()))
        .into_any_element()
}

fn notice(message: &str, colors: Colors) -> AnyElement {
    div()
        .rounded_lg()
        .bg(colors.raised)
        .p_3()
        .text_sm()
        .text_color(colors.muted)
        .child(message.to_string())
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_specific_parameters_must_be_an_object() {
        assert_eq!(parse_json_object("").unwrap(), Map::new());
        assert_eq!(
            parse_json_object(r#"{"reasoning_effort":"high"}"#).unwrap()["reasoning_effort"],
            "high"
        );
        assert!(parse_json_object("[]").is_err());
        assert!(parse_json_object("not json").is_err());
    }

    #[test]
    fn numeric_parameters_are_optional_and_finite() {
        assert_eq!(parse_optional::<u32>("Top K", "").unwrap(), None);
        assert_eq!(parse_optional::<u32>("Top K", "12").unwrap(), Some(12));
        assert!(parse_optional::<u32>("Top K", "-1").is_err());
        assert!(parse_optional_f64("Temperature", "NaN").is_err());
    }
}
