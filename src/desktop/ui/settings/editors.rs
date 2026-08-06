use super::*;

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

    pub(super) fn editing_id(&self) -> Option<&str> {
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
    pub(super) const CORE: [Self; 3] = [Self::Streaming, Self::Vision, Self::Thinking];

    pub(super) const PARAMETERS: [Self; 9] = [
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

    pub(super) fn label(self) -> &'static str {
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
