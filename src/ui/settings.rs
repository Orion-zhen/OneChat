use std::collections::BTreeMap;

use gpui::{
    AnyElement, App, Context, Div, ElementId, Entity, FontWeight, SharedString, Stateful, div,
    prelude::*, px, rgba,
};

use crate::{
    app::{ConnectionTestStatus, OneChat},
    model::{Model, ModelCapabilities, Provider, ProviderKind, now_timestamp},
    ui::{
        composer::Composer,
        shell::{Colors, button, compact_button, icon_button, primary_button},
    },
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum SettingsSection {
    #[default]
    General,
    SystemPrompts,
    Provider(String),
    NewProvider,
}

pub struct ProviderEditor {
    original: Option<Provider>,
    pub kind: ProviderKind,
    pub kind_menu_open: bool,
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
            kind_menu_open: false,
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

    pub fn toggle_kind_menu(&mut self) {
        self.kind_menu_open = !self.kind_menu_open;
    }

    pub fn select_kind(&mut self, kind: ProviderKind, cx: &mut Context<OneChat>) {
        self.kind_menu_open = false;
        if self.kind == kind {
            return;
        }

        let previous_default = self.kind.default_endpoint();
        self.kind = kind;
        let endpoint = self.endpoint.read(cx).text().trim().to_string();
        if endpoint.is_empty() || endpoint == previous_default {
            self.endpoint.update(cx, |input, cx| {
                input.set_text(kind.default_endpoint().to_string(), cx)
            });
        }
    }
}

pub struct ModelEditor {
    original: Option<Model>,
    provider_kind: ProviderKind,
    pub provider_id: String,
    pub remote_id: Entity<Composer>,
    pub display_name: Entity<Composer>,
    pub capabilities: ModelCapabilities,
}

impl ModelEditor {
    pub fn new(
        provider_id: String,
        provider_kind: ProviderKind,
        model: Option<Model>,
        cx: &mut Context<OneChat>,
    ) -> Self {
        let value = model
            .clone()
            .unwrap_or_else(|| Model::new_for_provider(&provider_id, "", "", provider_kind));
        Self {
            original: model,
            provider_kind,
            provider_id,
            remote_id: cx.new(|cx| Composer::single_line(value.remote_id, "Remote model ID", cx)),
            display_name: cx
                .new(|cx| Composer::single_line(value.display_name, "Display name", cx)),
            capabilities: value.capabilities,
        }
    }

    pub fn is_new(&self) -> bool {
        self.original.is_none()
    }

    fn editing_id(&self) -> Option<&str> {
        self.original.as_ref().map(|model| model.id.as_str())
    }

    pub fn build(&self, cx: &App) -> Result<Model, String> {
        let mut model = self.original.clone().unwrap_or_else(|| {
            Model::new_for_provider(&self.provider_id, "", "", self.provider_kind)
        });
        model.provider_id = self.provider_id.clone();
        model.remote_id = self.remote_id.read(cx).text().trim().to_string();
        if model.remote_id.is_empty() {
            return Err("Remote model ID is required.".into());
        }
        model.display_name = self.display_name.read(cx).text().trim().to_string();
        if model.display_name.is_empty() {
            model.display_name = model.remote_id.clone();
        }
        model.capabilities = self.capabilities.clone();
        model.updated_at = now_timestamp();
        Ok(model)
    }

    pub fn toggle_capability(&mut self, capability: Capability) {
        let value = match capability {
            Capability::Streaming => &mut self.capabilities.streaming,
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
    const CORE: [Self; 3] = [Self::Streaming, Self::Vision, Self::Thinking];

    const PARAMETERS: [Self; 9] = [
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
            Self::Vision => "Vision",
            Self::Thinking => "Thinking",
            Self::Temperature => "Temperature",
            Self::TopP => "Top P",
            Self::TopK => "Top K",
            Self::MaxOutputTokens => "Max Output",
            Self::FrequencyPenalty => "Frequency Penalty",
            Self::PresencePenalty => "Presence Penalty",
            Self::Seed => "Seed",
            Self::StopSequences => "Stop Sequences",
            Self::ThinkingBudget => "Thinking Budget",
        }
    }
}

pub(crate) fn render(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let detail = match &app.settings_section {
        SettingsSection::General => general_page(app, colors, cx),
        SettingsSection::SystemPrompts => system_prompts_page(app, colors, cx),
        SettingsSection::Provider(provider_id) => app
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == *provider_id)
            .map(|provider| provider_page(app, provider, colors, cx))
            .unwrap_or_else(|| general_page(app, colors, cx)),
        SettingsSection::NewProvider => new_provider_page(app, colors, cx),
    };

    div()
        .id("settings-page")
        .size_full()
        .min_w_0()
        .flex()
        .child(settings_sidebar(app, colors, cx))
        .child(detail)
        .into_any_element()
}

fn settings_sidebar(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let general_selected = app.settings_section == SettingsSection::General;
    let prompts_selected = app.settings_section == SettingsSection::SystemPrompts;
    let mut providers = div().flex().flex_col().gap_1();

    for provider in &app.snapshot.providers {
        let provider_id = provider.id.clone();
        let selected = app.settings_section == SettingsSection::Provider(provider.id.clone());
        let model_count = app
            .snapshot
            .models
            .iter()
            .filter(|model| model.provider_id == provider.id)
            .count();
        let status_color = match app.connection_tests.get(&provider.id) {
            Some(ConnectionTestStatus::Testing) => colors.accent,
            Some(ConnectionTestStatus::Connected) => colors.success,
            Some(ConnectionTestStatus::Failed(_)) => colors.danger,
            None if provider.enabled => colors.success,
            None => colors.muted,
        };
        providers = providers.child(
            provider_nav_row(provider, model_count, status_color, selected, colors).on_click(
                cx.listener(move |this, _, _, cx| {
                    this.select_settings_section(SettingsSection::Provider(provider_id.clone()), cx)
                }),
            ),
        );
    }

    if app.snapshot.providers.is_empty() {
        providers = providers.child(
            div()
                .px_3()
                .py_2()
                .text_size(px(12.0))
                .text_color(colors.muted)
                .child("No providers configured"),
        );
    }

    div()
        .w(px(244.0))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(colors.border)
        .bg(colors.sidebar)
        .child(
            div()
                .px_3()
                .pt_4()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    settings_nav_row(
                        "settings-general",
                        "⚙",
                        "General",
                        "Appearance and behavior",
                        general_selected,
                        colors,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_settings_section(SettingsSection::General, cx)
                    })),
                )
                .child(
                    settings_nav_row(
                        "settings-system-prompts",
                        "✦",
                        "System Prompts",
                        "Default instructions",
                        prompts_selected,
                        colors,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_settings_section(SettingsSection::SystemPrompts, cx)
                    })),
                ),
        )
        .child(
            div()
                .px_5()
                .pt_6()
                .pb_2()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors.muted)
                        .child("PROVIDERS"),
                )
                .child(
                    icon_button("add-provider-sidebar", "+", colors)
                        .size(px(26.0))
                        .on_click(cx.listener(|this, _, _, cx| this.begin_add_provider(cx))),
                ),
        )
        .child(
            div()
                .id("settings-provider-list")
                .min_h_0()
                .flex_1()
                .overflow_y_scroll()
                .px_3()
                .child(providers),
        )
        .child(
            div().border_t_1().border_color(colors.border).p_3().child(
                settings_nav_row(
                    "settings-add-provider",
                    "+",
                    "Add Provider",
                    "Connect another service",
                    app.settings_section == SettingsSection::NewProvider,
                    colors,
                )
                .on_click(cx.listener(|this, _, _, cx| this.begin_add_provider(cx))),
            ),
        )
        .into_any_element()
}

fn settings_nav_row(
    id: impl Into<ElementId>,
    icon: &'static str,
    title: &'static str,
    detail: &'static str,
    selected: bool,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .rounded_lg()
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .gap_3()
        .bg(if selected {
            colors.accent_soft
        } else {
            rgba(0x00000000)
        })
        .cursor_pointer()
        .hover(move |style| {
            style.bg(if selected {
                colors.accent_soft
            } else {
                colors.hover
            })
        })
        .active(move |style| style.bg(colors.accent_soft))
        .child(
            div()
                .w(px(22.0))
                .flex_none()
                .text_center()
                .text_size(px(15.0))
                .text_color(if selected {
                    colors.accent
                } else {
                    colors.muted
                })
                .child(icon),
        )
        .child(
            div()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .font_weight(if selected {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .child(title),
                )
                .child(
                    div()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(11.0))
                        .text_color(colors.muted)
                        .child(detail),
                ),
        )
}

fn provider_nav_row(
    provider: &Provider,
    model_count: usize,
    status_color: gpui::Rgba,
    selected: bool,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!(
            "settings-provider-{}",
            provider.id
        )))
        .rounded_lg()
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .gap_3()
        .bg(if selected {
            colors.accent_soft
        } else {
            rgba(0x00000000)
        })
        .cursor_pointer()
        .hover(move |style| {
            style.bg(if selected {
                colors.accent_soft
            } else {
                colors.hover
            })
        })
        .active(move |style| style.bg(colors.accent_soft))
        .child(
            div()
                .w(px(22.0))
                .flex_none()
                .flex()
                .justify_center()
                .child(div().size(px(8.0)).rounded_full().bg(status_color)),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_sm()
                        .font_weight(if selected {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .child(provider.name.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(colors.muted)
                        .child(format!(
                            "{} · {} {}",
                            provider.kind.label(),
                            model_count,
                            if model_count == 1 { "model" } else { "models" }
                        )),
                ),
        )
}

fn general_page(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let theme = app.settings().theme.label();
    let appearance = div().flex().flex_col().gap_2().child(setting_row(
        "Theme",
        "Match the Mac or choose a fixed appearance.",
        button("cycle-theme", theme, colors)
            .on_click(cx.listener(|this, _, _, cx| this.cycle_theme(cx))),
        colors,
    ));

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "General",
                "Choose how OneChat looks and responds.",
                colors,
            ))
            .child(section("Appearance", None, appearance, colors)),
    )
}

fn system_prompts_page(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let content = if let Some(editor) = &app.default_system_prompt_editor {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(editor.clone())
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        button("cancel-default-system-prompt", "Cancel", colors).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.cancel_default_system_prompt_edit(cx)
                            }),
                        ),
                    )
                    .child(
                        primary_button("save-default-system-prompt", "Save", colors).on_click(
                            cx.listener(|this, _, _, cx| this.save_default_system_prompt(cx)),
                        ),
                    ),
            )
            .into_any_element()
    } else {
        let prompt = app.snapshot.settings.default_system_prompt.trim();
        div()
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
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Default Prompt"),
                    )
                    .child(
                        div()
                            .pt_1()
                            .text_size(px(12.0))
                            .line_height(px(18.0))
                            .text_color(colors.muted)
                            .child(if prompt.is_empty() {
                                "New conversations start without a System Prompt.".into()
                            } else {
                                prompt_preview(prompt)
                            }),
                    ),
            )
            .child(
                button("edit-default-system-prompt", "Edit", colors).on_click(
                    cx.listener(|this, _, _, cx| this.begin_edit_default_system_prompt(cx)),
                ),
            )
            .into_any_element()
    };

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "System Prompts",
                "Set the instructions copied into every new conversation.",
                colors,
            ))
            .child(section(
                "Default",
                Some("Existing conversations keep their own prompt."),
                content,
                colors,
            )),
    )
}

fn new_provider_page(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let content = app.provider_editor.as_ref().map_or_else(
        || {
            div()
                .text_sm()
                .text_color(colors.muted)
                .child("Preparing provider settings…")
                .into_any_element()
        },
        |editor| provider_form(editor, colors, cx),
    );

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "Add Provider",
                "Connect OneChat to an LLM service.",
                colors,
            ))
            .children(
                app.form_error
                    .as_ref()
                    .map(|error| error_banner(error, colors)),
            )
            .child(content),
    )
}

fn provider_page(
    app: &OneChat,
    provider: &Provider,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let (status, status_color) = provider_status(app, provider, colors);
    let provider_id = provider.id.clone();
    let edit_id = provider.id.clone();
    let testing = matches!(
        app.connection_tests.get(&provider.id),
        Some(ConnectionTestStatus::Testing)
    );
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
                        .child(provider.name.clone()),
                )
                .child(
                    div()
                        .pt_1()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_sm()
                        .text_color(colors.muted)
                        .child(div().size(px(7.0)).rounded_full().bg(status_color))
                        .child(format!("{} · {status}", provider.kind.label())),
                ),
        )
        .when(app.provider_editor.is_none(), |element| {
            element.child(
                div()
                    .flex_none()
                    .flex()
                    .gap_2()
                    .child(
                        button(
                            SharedString::from(format!("test-provider-{}", provider.id)),
                            if testing {
                                "Testing…"
                            } else {
                                "Test Connection"
                            },
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.test_provider_connection(provider_id.clone(), cx)
                        })),
                    )
                    .child(
                        primary_button(
                            SharedString::from(format!("edit-provider-{}", provider.id)),
                            "Edit",
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.begin_edit_provider(edit_id.clone(), cx)
                        })),
                    ),
            )
        });

    let body = if let Some(editor) = &app.provider_editor {
        provider_form(editor, colors, cx)
    } else {
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(provider_summary(provider, colors))
            .child(provider_models(app, provider, colors, cx))
            .child(provider_danger_zone(provider, colors, cx))
            .into_any_element()
    };

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(header)
            .children(
                app.form_error
                    .as_ref()
                    .map(|error| error_banner(error, colors)),
            )
            .child(body),
    )
}

fn provider_status(app: &OneChat, provider: &Provider, colors: Colors) -> (String, gpui::Rgba) {
    match app.connection_tests.get(&provider.id) {
        Some(ConnectionTestStatus::Testing) => ("Testing connection…".into(), colors.accent),
        Some(ConnectionTestStatus::Connected) => ("Connected".into(), colors.success),
        Some(ConnectionTestStatus::Failed(message)) => {
            (format!("Connection failed: {message}"), colors.danger)
        }
        None if provider.enabled => ("Enabled".into(), colors.success),
        None => ("Disabled".into(), colors.muted),
    }
}

fn provider_summary(provider: &Provider, colors: Colors) -> AnyElement {
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
        .flex()
        .flex_col()
        .gap_2()
        .child(summary_row("Endpoint", provider.endpoint.clone(), colors))
        .child(summary_row("API Key", api_key, colors))
        .child(summary_row("Custom Headers", headers, colors))
        .child(summary_row("Proxy", proxy, colors));
    section(
        "Connection",
        Some("Credentials are stored as plain text on this Mac."),
        content,
        colors,
    )
}

fn provider_danger_zone(
    provider: &Provider,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let provider_id = provider.id.clone();
    let content = div()
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
                        .child("Delete Provider"),
                )
                .child(
                    div()
                        .pt_1()
                        .text_size(px(12.0))
                        .text_color(colors.muted)
                        .child("This also removes every model configured for this provider."),
                ),
        )
        .child(
            button("delete-provider", "Delete…", colors)
                .text_color(colors.danger)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.request_delete_provider(provider_id.clone(), cx)
                })),
        );
    section("Danger Zone", None, content, colors)
}

fn provider_models(
    app: &OneChat,
    provider: &Provider,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let editor = app
        .model_editor
        .as_ref()
        .filter(|editor| editor.provider_id == provider.id);
    let editing_id = editor.and_then(ModelEditor::editing_id);
    let provider_id = provider.id.clone();
    let mut models = div().flex().flex_col().gap_2();

    if let Some(editor) = editor {
        models = models.child(model_form(editor, colors, cx));
    }

    let configured_models = app
        .snapshot
        .models
        .iter()
        .filter(|model| model.provider_id == provider.id)
        .filter(|model| editing_id != Some(model.id.as_str()))
        .collect::<Vec<_>>();

    if configured_models.is_empty() && editor.is_none() {
        models = models.child(
            div()
                .rounded_lg()
                .bg(colors.raised)
                .p_5()
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
                        .text_color(colors.muted)
                        .child("Add a remote model ID to use this provider in conversations."),
                ),
        );
    }

    for model in configured_models {
        models = models.child(model_row(model, colors, cx));
    }

    let header = div()
        .flex()
        .items_end()
        .justify_between()
        .gap_4()
        .child(
            div()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Models"),
                )
                .child(
                    div()
                        .pt_1()
                        .text_size(px(12.0))
                        .text_color(colors.muted)
                        .child("Models are configured and managed within this provider."),
                ),
        )
        .when(editor.is_none(), |element| {
            element.child(primary_button("add-model", "Add Model", colors).on_click(
                cx.listener(move |this, _, _, cx| this.begin_add_model(provider_id.clone(), cx)),
            ))
        });

    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(header)
        .child(
            div()
                .rounded_xl()
                .border_1()
                .border_color(colors.border)
                .bg(colors.panel)
                .p_4()
                .child(models),
        )
        .into_any_element()
}

fn model_row(model: &Model, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let edit_id = model.id.clone();
    let delete_id = model.id.clone();
    div()
        .rounded_lg()
        .bg(colors.raised)
        .px_4()
        .py_3()
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
                        .child(model.display_name.clone()),
                )
                .child(
                    div()
                        .pt_1()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(11.0))
                        .text_color(colors.muted)
                        .child(format!(
                            "{} · {}",
                            model.remote_id,
                            model_capability_summary(&model.capabilities)
                        )),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .gap_1()
                .child(
                    compact_button(
                        SharedString::from(format!("edit-model-{}", model.id)),
                        "Edit",
                        colors,
                    )
                    .on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.begin_edit_model(edit_id.clone(), cx)
                        }),
                    ),
                )
                .child(
                    compact_button(
                        SharedString::from(format!("delete-model-{}", model.id)),
                        "Delete",
                        colors,
                    )
                    .text_color(colors.danger)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request_delete_model(delete_id.clone(), cx)
                    })),
                ),
        )
        .into_any_element()
}

fn model_capability_summary(capabilities: &ModelCapabilities) -> String {
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
        "No core capabilities".into()
    } else {
        labels.join(", ")
    }
}

fn provider_kind_select(
    editor: &ProviderEditor,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let mut options = div()
        .w_full()
        .mt_1()
        .rounded_lg()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .p_1()
        .flex()
        .flex_col()
        .shadow_md();
    for kind in ProviderKind::ALL {
        let selected = kind == editor.kind;
        options = options.child(
            div()
                .id(SharedString::from(format!(
                    "provider-kind-option-{}",
                    kind.as_str()
                )))
                .w_full()
                .px_3()
                .py_2()
                .rounded_md()
                .flex()
                .items_center()
                .justify_between()
                .bg(if selected {
                    colors.accent_soft
                } else {
                    colors.panel
                })
                .text_sm()
                .text_color(if selected { colors.accent } else { colors.text })
                .cursor_pointer()
                .hover(move |style| style.bg(colors.hover))
                .on_click(cx.listener(move |this, _, _, cx| this.select_provider_kind(kind, cx)))
                .child(kind.label())
                .children(selected.then(|| div().text_color(colors.accent).child("✓"))),
        );
    }

    div()
        .min_w(px(240.0))
        .flex_1()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.muted)
                .child("Type"),
        )
        .child(
            div()
                .id("provider-kind-select")
                .w_full()
                .h(px(36.0))
                .px_3()
                .rounded_lg()
                .border_1()
                .border_color(colors.border)
                .bg(colors.raised)
                .flex()
                .items_center()
                .justify_between()
                .text_sm()
                .cursor_pointer()
                .hover(move |style| style.bg(colors.hover))
                .on_click(cx.listener(|this, _, _, cx| this.toggle_provider_kind_menu(cx)))
                .child(editor.kind.label())
                .child(
                    div()
                        .text_color(colors.muted)
                        .child(if editor.kind_menu_open { "⌃" } else { "⌄" }),
                ),
        )
        .children(editor.kind_menu_open.then_some(options))
        .into_any_element()
}

fn provider_form(editor: &ProviderEditor, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let identity = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(field("Name", editor.name.clone(), colors))
        .child(
            div()
                .flex()
                .items_start()
                .gap_2()
                .child(provider_kind_select(editor, colors, cx))
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(colors.muted)
                                .child("Status"),
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
                            .when(editor.enabled, |element| {
                                element.bg(colors.accent_soft).text_color(colors.accent)
                            })
                            .on_click(
                                cx.listener(|this, _, _, cx| this.toggle_provider_enabled(cx)),
                            ),
                        ),
                ),
        );
    let connection = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(field("Endpoint", editor.endpoint.clone(), colors))
        .child(field(
            "API Key · stored as plain text",
            editor.api_key.clone(),
            colors,
        ));
    let advanced = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(field(
            "Custom Headers · JSON",
            editor.headers.clone(),
            colors,
        ))
        .child(field("Proxy", editor.proxy.clone(), colors));

    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(section("Provider", None, identity, colors))
        .child(section("Connection", None, connection, colors))
        .child(section(
            "Advanced",
            Some("Optional request headers and proxy routing."),
            advanced,
            colors,
        ))
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    button("cancel-provider", "Cancel", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_provider_editor(cx))),
                )
                .child(
                    primary_button(
                        "save-provider",
                        if editor.is_new() {
                            "Add Provider"
                        } else {
                            "Save Changes"
                        },
                        colors,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.save_provider(cx))),
                ),
        )
        .into_any_element()
}

fn model_form(editor: &ModelEditor, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let title = if editor.is_new() {
        "Add Model"
    } else {
        "Edit Model"
    };
    div()
        .rounded_lg()
        .bg(colors.raised)
        .p_4()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(field("Remote Model ID", editor.remote_id.clone(), colors))
        .child(field("Display Name", editor.display_name.clone(), colors))
        .child(capability_group(
            "Core Capabilities",
            &Capability::CORE,
            editor,
            colors,
            cx,
        ))
        .child(capability_group(
            "Supported Parameters",
            &Capability::PARAMETERS,
            editor,
            colors,
            cx,
        ))
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    button("cancel-model", "Cancel", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_model_editor(cx))),
                )
                .child(
                    primary_button("save-model", "Save Model", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.save_model(cx))),
                ),
        )
        .into_any_element()
}

fn capability_group(
    title: &'static str,
    capabilities: &'static [Capability],
    editor: &ModelEditor,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let mut toggles = div().flex().flex_wrap().gap_2();
    for capability in capabilities {
        let capability = *capability;
        let enabled = editor.capability(capability);
        toggles = toggles.child(
            button(
                SharedString::from(format!("capability-{capability:?}")),
                capability.label(),
                colors,
            )
            .when(enabled, |element| {
                element.bg(colors.accent_soft).text_color(colors.accent)
            })
            .when(!enabled, |element| element.text_color(colors.muted))
            .on_click(
                cx.listener(move |this, _, _, cx| this.toggle_model_capability(capability, cx)),
            ),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.muted)
                .child(title),
        )
        .child(toggles)
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
