use std::collections::BTreeMap;

use gpui::{AnyElement, App, Context, Entity, FontWeight, SharedString, div, prelude::*, px};

use crate::{
    app::{ConnectionTestStatus, OneChat},
    model::{GenerationConfig, Model, ModelCapabilities, Provider, ProviderKind, now_timestamp},
    ui::{
        composer::Composer,
        shell::{Colors, button},
    },
};

pub struct ProviderEditor {
    original: Option<Provider>,
    pub kind: ProviderKind,
    pub enabled: bool,
    pub name: Entity<Composer>,
    pub endpoint: Entity<Composer>,
    pub api_key: Entity<Composer>,
    pub headers: Entity<Composer>,
    pub proxy: Entity<Composer>,
}

impl ProviderEditor {
    pub fn new(provider: Option<Provider>, cx: &mut Context<OneChat>) -> Self {
        let value = provider
            .clone()
            .unwrap_or_else(|| Provider::new("", ProviderKind::OpenAi));
        let headers = serde_json::to_string_pretty(&value.headers).unwrap_or_else(|_| "{}".into());
        Self {
            original: provider,
            kind: value.kind,
            enabled: value.enabled,
            name: cx.new(|cx| Composer::single_line(value.name, "Provider name", cx)),
            endpoint: cx.new(|cx| Composer::single_line(value.endpoint, "Endpoint", cx)),
            api_key: cx.new(|cx| Composer::single_line(value.api_key, "API key", cx)),
            headers: cx.new(|cx| Composer::multiline(headers, "Custom headers JSON", cx)),
            proxy: cx.new(|cx| {
                Composer::single_line(value.proxy.unwrap_or_default(), "Optional proxy URL", cx)
            }),
        }
    }

    pub fn is_new(&self) -> bool {
        self.original.is_none()
    }

    pub fn build(&self, cx: &App) -> Result<Provider, String> {
        let mut provider = self
            .original
            .clone()
            .unwrap_or_else(|| Provider::new("", self.kind));
        provider.name = self.name.read(cx).text().trim().to_string();
        if provider.name.is_empty() {
            return Err("Provider name is required.".into());
        }
        provider.kind = self.kind;
        provider.endpoint = self.endpoint.read(cx).text().trim().to_string();
        if provider.endpoint.is_empty() && self.kind.default_endpoint().is_empty() {
            return Err("Endpoint is required for an OpenAI-compatible provider.".into());
        }
        provider.api_key = self.api_key.read(cx).text().trim().to_string();
        provider.headers = parse_headers(self.headers.read(cx).text())?;
        provider.proxy = nonempty(self.proxy.read(cx).text());
        provider.enabled = self.enabled;
        provider.updated_at = now_timestamp();
        Ok(provider)
    }

    pub fn cycle_kind(&mut self, cx: &mut Context<OneChat>) {
        let previous_default = self.kind.default_endpoint();
        self.kind = self.kind.next();
        let endpoint = self.endpoint.read(cx).text().trim().to_string();
        if endpoint.is_empty() || endpoint == previous_default {
            let next = self.kind.default_endpoint().to_string();
            self.endpoint
                .update(cx, |input, cx| input.set_text(next, cx));
        }
    }
}

pub struct ModelEditor {
    original: Option<Model>,
    pub provider_id: String,
    pub remote_id: Entity<Composer>,
    pub display_name: Entity<Composer>,
    pub default_config: Entity<Composer>,
    pub capabilities: ModelCapabilities,
}

impl ModelEditor {
    pub fn new(provider_id: String, model: Option<Model>, cx: &mut Context<OneChat>) -> Self {
        let value = model
            .clone()
            .unwrap_or_else(|| Model::new(&provider_id, "", ""));
        let config =
            serde_json::to_string_pretty(&value.default_config).unwrap_or_else(|_| "{}".into());
        Self {
            original: model,
            provider_id,
            remote_id: cx.new(|cx| Composer::single_line(value.remote_id, "Remote model ID", cx)),
            display_name: cx
                .new(|cx| Composer::single_line(value.display_name, "Display name", cx)),
            default_config: cx
                .new(|cx| Composer::multiline(config, "Default generation parameters JSON", cx)),
            capabilities: value.capabilities,
        }
    }

    pub fn is_new(&self) -> bool {
        self.original.is_none()
    }

    pub fn build(&self, cx: &App) -> Result<Model, String> {
        let mut model = self
            .original
            .clone()
            .unwrap_or_else(|| Model::new(&self.provider_id, "", ""));
        model.provider_id = self.provider_id.clone();
        model.remote_id = self.remote_id.read(cx).text().trim().to_string();
        if model.remote_id.is_empty() {
            return Err("Remote model ID is required.".into());
        }
        model.display_name = self.display_name.read(cx).text().trim().to_string();
        if model.display_name.is_empty() {
            model.display_name = model.remote_id.clone();
        }
        model.default_config =
            serde_json::from_str::<GenerationConfig>(self.default_config.read(cx).text().trim())
                .map_err(|error| format!("Invalid default parameters JSON: {error}"))?;
        model.capabilities = self.capabilities.clone();
        model.updated_at = now_timestamp();
        Ok(model)
    }

    pub fn toggle_capability(&mut self, capability: Capability) {
        let value = match capability {
            Capability::Streaming => &mut self.capabilities.streaming,
            Capability::SystemPrompt => &mut self.capabilities.system_prompt,
            Capability::Vision => &mut self.capabilities.vision,
            Capability::Thinking => &mut self.capabilities.thinking,
            Capability::Temperature => &mut self.capabilities.temperature,
            Capability::TopP => &mut self.capabilities.top_p,
            Capability::TopK => &mut self.capabilities.top_k,
            Capability::MaxOutputTokens => &mut self.capabilities.max_output_tokens,
            Capability::FrequencyPenalty => &mut self.capabilities.frequency_penalty,
            Capability::PresencePenalty => &mut self.capabilities.presence_penalty,
            Capability::Seed => &mut self.capabilities.seed,
            Capability::StopSequences => &mut self.capabilities.stop_sequences,
            Capability::ThinkingBudget => &mut self.capabilities.thinking_budget,
        };
        *value = !*value;
    }

    pub fn capability(&self, capability: Capability) -> bool {
        match capability {
            Capability::Streaming => self.capabilities.streaming,
            Capability::SystemPrompt => self.capabilities.system_prompt,
            Capability::Vision => self.capabilities.vision,
            Capability::Thinking => self.capabilities.thinking,
            Capability::Temperature => self.capabilities.temperature,
            Capability::TopP => self.capabilities.top_p,
            Capability::TopK => self.capabilities.top_k,
            Capability::MaxOutputTokens => self.capabilities.max_output_tokens,
            Capability::FrequencyPenalty => self.capabilities.frequency_penalty,
            Capability::PresencePenalty => self.capabilities.presence_penalty,
            Capability::Seed => self.capabilities.seed,
            Capability::StopSequences => self.capabilities.stop_sequences,
            Capability::ThinkingBudget => self.capabilities.thinking_budget,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Capability {
    Streaming,
    SystemPrompt,
    Vision,
    Thinking,
    Temperature,
    TopP,
    TopK,
    MaxOutputTokens,
    FrequencyPenalty,
    PresencePenalty,
    Seed,
    StopSequences,
    ThinkingBudget,
}

impl Capability {
    const ALL: [Self; 13] = [
        Self::Streaming,
        Self::SystemPrompt,
        Self::Vision,
        Self::Thinking,
        Self::Temperature,
        Self::TopP,
        Self::TopK,
        Self::MaxOutputTokens,
        Self::FrequencyPenalty,
        Self::PresencePenalty,
        Self::Seed,
        Self::StopSequences,
        Self::ThinkingBudget,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Streaming => "Streaming",
            Self::SystemPrompt => "System prompt",
            Self::Vision => "Vision",
            Self::Thinking => "Thinking",
            Self::Temperature => "Temperature",
            Self::TopP => "Top P",
            Self::TopK => "Top K",
            Self::MaxOutputTokens => "Max output",
            Self::FrequencyPenalty => "Frequency penalty",
            Self::PresencePenalty => "Presence penalty",
            Self::Seed => "Seed",
            Self::StopSequences => "Stop sequences",
            Self::ThinkingBudget => "Thinking budget",
        }
    }
}

pub(crate) fn render(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let theme = app.settings().theme.label();
    let reduce_motion = if app.settings().reduce_motion {
        "On"
    } else {
        "Off"
    };
    let mut content = div()
        .mx_auto()
        .w_full()
        .max_w(px(820.0))
        .flex()
        .flex_col()
        .gap_5()
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Settings"),
        )
        .child(card(
            "Appearance",
            div()
                .flex()
                .gap_2()
                .child(
                    button("cycle-theme", format!("Theme: {theme}"), colors)
                        .on_click(cx.listener(|this, _, _, cx| this.cycle_theme(cx))),
                )
                .child(
                    button(
                        "toggle-reduce-motion",
                        format!("Reduce Motion: {reduce_motion}"),
                        colors,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_reduce_motion(cx))),
                ),
            colors,
        ))
        .child(provider_section(app, colors, cx));

    if let Some(error) = &app.form_error {
        content = content.child(
            div()
                .rounded_lg()
                .bg(colors.raised)
                .p_3()
                .text_color(colors.danger)
                .child(error.clone()),
        );
    }
    content = content.child(model_section(app, colors, cx));

    div()
        .id("settings-page")
        .min_w_0()
        .flex_1()
        .h_full()
        .overflow_y_scroll()
        .p_6()
        .child(content)
        .into_any_element()
}

fn provider_section(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let mut body = div().flex().flex_col().gap_3();
    if let Some(editor) = &app.provider_editor {
        body = body.child(provider_form(editor, colors, cx));
    } else {
        body = body.child(
            button("add-provider", "+ Add provider", colors)
                .on_click(cx.listener(|this, _, _, cx| this.begin_add_provider(cx))),
        );
    }
    if app.snapshot.providers.is_empty() {
        body = body.child(
            div()
                .text_sm()
                .text_color(colors.muted)
                .child("No providers configured."),
        );
    }
    for provider in &app.snapshot.providers {
        let edit_id = provider.id.clone();
        let delete_id = provider.id.clone();
        let test_id = provider.id.clone();
        let status = match app.connection_tests.get(&provider.id) {
            Some(ConnectionTestStatus::Testing) => "Testing…".to_string(),
            Some(ConnectionTestStatus::Connected) => "Connected".to_string(),
            Some(ConnectionTestStatus::Failed(message)) => format!("Failed: {message}"),
            None => if provider.enabled {
                "Enabled"
            } else {
                "Disabled"
            }
            .into(),
        };
        body = body.child(
            div()
                .rounded_lg()
                .border_1()
                .border_color(colors.border)
                .p_3()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .min_w_0()
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(provider.name.clone()),
                        )
                        .child(div().text_xs().text_color(colors.muted).child(format!(
                            "{} · {}",
                            provider.kind.label(),
                            status
                        )))
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.muted)
                                .child(provider.endpoint.clone()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            button(
                                SharedString::from(format!("test-{}", provider.id)),
                                "Test",
                                colors,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.test_provider_connection(test_id.clone(), cx)
                                },
                            )),
                        )
                        .child(
                            button(
                                SharedString::from(format!("edit-{}", provider.id)),
                                "Edit",
                                colors,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| this.begin_edit_provider(edit_id.clone(), cx),
                            )),
                        )
                        .child(
                            button(
                                SharedString::from(format!("delete-{}", provider.id)),
                                "Delete",
                                colors,
                            )
                            .text_color(colors.danger)
                            .on_click(cx.listener(
                                move |this, _, _, cx| this.delete_provider(delete_id.clone(), cx),
                            )),
                        ),
                ),
        );
    }
    card("Providers", body, colors)
}

fn provider_form(editor: &ProviderEditor, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    div()
        .rounded_lg()
        .bg(colors.raised)
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(field("Name", editor.name.clone(), colors))
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    button(
                        "provider-kind",
                        format!("Type: {}", editor.kind.label()),
                        colors,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.cycle_provider_kind(cx))),
                )
                .child(
                    button(
                        "provider-enabled",
                        if editor.enabled {
                            "Enabled"
                        } else {
                            "Disabled"
                        },
                        colors,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_provider_enabled(cx))),
                ),
        )
        .child(field("Endpoint", editor.endpoint.clone(), colors))
        .child(field(
            "API key (stored as plain text)",
            editor.api_key.clone(),
            colors,
        ))
        .child(field("Custom headers JSON", editor.headers.clone(), colors))
        .child(field("Proxy", editor.proxy.clone(), colors))
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    button("save-provider", "Save provider", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.save_provider(cx))),
                )
                .child(
                    button("cancel-provider", "Cancel", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_provider_editor(cx))),
                ),
        )
        .into_any_element()
}

fn model_section(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let mut body = div().flex().flex_col().gap_3();
    if let Some(editor) = &app.model_editor {
        body = body.child(model_form(editor, colors, cx));
    } else if app.snapshot.providers.is_empty() {
        body = body.child(
            div()
                .text_sm()
                .text_color(colors.muted)
                .child("Add a provider before adding models."),
        );
    } else {
        for provider in &app.snapshot.providers {
            let provider_id = provider.id.clone();
            body =
                body.child(
                    button(
                        SharedString::from(format!("add-model-{}", provider.id)),
                        format!("+ Add model to {}", provider.name),
                        colors,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.begin_add_model(provider_id.clone(), cx)
                    })),
                );
        }
    }
    for model in &app.snapshot.models {
        let edit_id = model.id.clone();
        let delete_id = model.id.clone();
        let provider = app
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == model.provider_id)
            .map(|provider| provider.name.as_str())
            .unwrap_or("Missing provider");
        body = body.child(
            div()
                .rounded_lg()
                .border_1()
                .border_color(colors.border)
                .p_3()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(model.display_name.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.muted)
                                .child(format!("{} · {provider}", model.remote_id)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            button(
                                SharedString::from(format!("edit-model-{}", model.id)),
                                "Edit",
                                colors,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| this.begin_edit_model(edit_id.clone(), cx),
                            )),
                        )
                        .child(
                            button(
                                SharedString::from(format!("delete-model-{}", model.id)),
                                "Delete",
                                colors,
                            )
                            .text_color(colors.danger)
                            .on_click(cx.listener(
                                move |this, _, _, cx| this.delete_model(delete_id.clone(), cx),
                            )),
                        ),
                ),
        );
    }
    card("Models", body, colors)
}

fn model_form(editor: &ModelEditor, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let mut capabilities = div().flex().flex_wrap().gap_2();
    for capability in Capability::ALL {
        let enabled = editor.capability(capability);
        capabilities = capabilities.child(
            button(
                SharedString::from(format!("capability-{capability:?}")),
                format!(
                    "{}: {}",
                    capability.label(),
                    if enabled { "On" } else { "Off" }
                ),
                colors,
            )
            .on_click(
                cx.listener(move |this, _, _, cx| this.toggle_model_capability(capability, cx)),
            ),
        );
    }
    div()
        .rounded_lg()
        .bg(colors.raised)
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(field("Remote model ID", editor.remote_id.clone(), colors))
        .child(field("Display name", editor.display_name.clone(), colors))
        .child(field(
            "Default raw generation parameters JSON",
            editor.default_config.clone(),
            colors,
        ))
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Capabilities"),
        )
        .child(capabilities)
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    button("save-model", "Save model", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.save_model(cx))),
                )
                .child(
                    button("cancel-model", "Cancel", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_model_editor(cx))),
                ),
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
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.muted)
                .child(label.to_string()),
        )
        .child(input)
        .into_any_element()
}

fn card(title: &str, content: impl IntoElement, colors: Colors) -> AnyElement {
    div()
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .p_5()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_string()),
        )
        .child(content)
        .into_any_element()
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
}
