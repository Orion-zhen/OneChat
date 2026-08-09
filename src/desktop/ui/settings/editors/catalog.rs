use super::*;

pub struct ProviderEditor {
    original: Option<Provider>,
    pub kind: Entity<SelectState<Vec<ProviderKindItem>>>,
    pub name: Entity<InputState>,
    pub endpoint: Entity<InputState>,
    pub api_key: Entity<InputState>,
    pub headers: Entity<InputState>,
    pub proxy: Entity<InputState>,
}

impl ProviderEditor {
    pub fn new(provider: Option<Provider>, window: &mut Window, cx: &mut Context<OneChat>) -> Self {
        let value = provider
            .clone()
            .unwrap_or_else(|| Provider::new("", ProviderKind::OpenAi));
        let selected = ProviderKind::ALL
            .iter()
            .position(|kind| *kind == value.kind)
            .map(IndexPath::new);
        let headers = serde_json::to_string_pretty(&value.headers).unwrap_or_else(|_| "{}".into());
        Self {
            original: provider,
            kind: cx.new(|cx| {
                SelectState::new(
                    ProviderKind::ALL
                        .into_iter()
                        .map(ProviderKindItem::new)
                        .collect(),
                    selected,
                    window,
                    cx,
                )
            }),
            name: single_line_input(value.name, "Provider name", window, cx),
            endpoint: single_line_input(value.endpoint, "Endpoint", window, cx),
            api_key: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(value.api_key)
                    .placeholder("API key")
                    .masked(true)
            }),
            headers: multiline_input(headers, "Custom headers JSON", window, cx),
            proxy: single_line_input(
                value.proxy.unwrap_or_default(),
                "Optional proxy URL",
                window,
                cx,
            ),
        }
    }

    pub fn is_new(&self) -> bool {
        self.original.is_none()
    }

    pub fn kind(&self, cx: &App) -> ProviderKind {
        self.kind
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or_default()
    }

    pub fn build(&self, cx: &App) -> Result<Provider, String> {
        let kind = self.kind(cx);
        let mut provider = self
            .original
            .clone()
            .unwrap_or_else(|| Provider::new("", kind));
        provider.name = self.name.read(cx).value().trim().to_string();
        if provider.name.is_empty() {
            return Err("Provider name is required.".into());
        }
        provider.kind = kind;
        provider.endpoint = self.endpoint.read(cx).value().trim().to_string();
        if provider.endpoint.is_empty() && kind.default_endpoint().is_empty() {
            return Err("Endpoint is required for an OpenAI-compatible provider.".into());
        }
        provider.api_key = self.api_key.read(cx).value().trim().to_string();
        provider.headers = parse_headers(self.headers.read(cx).value().as_ref())?;
        provider.proxy = nonempty(self.proxy.read(cx).value().as_ref());
        provider.updated_at = now_timestamp();
        Ok(provider)
    }

    pub fn select_kind(
        &mut self,
        kind: ProviderKind,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) {
        let endpoint = self.endpoint.read(cx).value().trim().to_string();
        let uses_known_default = ProviderKind::ALL
            .into_iter()
            .any(|candidate| endpoint == candidate.default_endpoint());
        if endpoint.is_empty() || uses_known_default {
            self.endpoint.update(cx, |input, cx| {
                input.set_value(kind.default_endpoint(), window, cx)
            });
        }
    }
}

#[derive(Clone, Debug)]
pub enum ModelFetchStatus {
    Loading,
    Loaded,
    Failed(String),
}

pub struct ModelEditor {
    original: Option<Model>,
    provider_kind: ProviderKind,
    last_remote_id: String,
    pub provider_id: String,
    pub remote_id: Entity<ComboboxState<ModelIdDelegate>>,
    pub display_name: Entity<InputState>,
    pub capabilities: ModelCapabilities,
    pub reasoning: ModelReasoningEditor,
    pub available_models: Vec<AvailableModel>,
    pub fetch_status: ModelFetchStatus,
    synced_models: Vec<AvailableModel>,
    synced_remote_id: String,
}

impl ModelEditor {
    pub fn new(
        provider_id: String,
        provider_kind: ProviderKind,
        model: Option<Model>,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        let value = model
            .clone()
            .unwrap_or_else(|| Model::new_for_provider(&provider_id, "", "", provider_kind));
        let remote_id = value.remote_id.clone();
        let selected = (!remote_id.is_empty()).then(|| IndexPath::new(0));
        let reasoning = ModelReasoningEditor::new(
            value.reasoning.clone(),
            default_reasoning_format(provider_kind, &remote_id),
            window,
            cx,
        );
        Self {
            original: model,
            provider_kind,
            last_remote_id: remote_id.clone(),
            provider_id,
            remote_id: cx.new(|cx| {
                ComboboxState::new(
                    ModelIdDelegate::new(&remote_id, &[]),
                    selected.into_iter().collect(),
                    window,
                    cx,
                )
                .searchable(true)
            }),
            display_name: single_line_input(value.display_name, "Display name", window, cx),
            capabilities: value.capabilities,
            reasoning,
            available_models: Vec::new(),
            fetch_status: ModelFetchStatus::Loading,
            synced_models: Vec::new(),
            synced_remote_id: remote_id,
        }
    }

    pub fn is_new(&self) -> bool {
        self.original.is_none()
    }

    pub(crate) fn editing_id(&self) -> Option<&str> {
        self.original.as_ref().map(|model| model.id.as_str())
    }

    pub fn remote_id(&self, cx: &App) -> String {
        self.remote_id.read(cx).selected_value().unwrap_or_default()
    }

    pub fn build(&self, cx: &App) -> Result<Model, String> {
        let mut model = self.original.clone().unwrap_or_else(|| {
            Model::new_for_provider(&self.provider_id, "", "", self.provider_kind)
        });
        model.provider_id = self.provider_id.clone();
        model.remote_id = self.remote_id(cx).trim().to_string();
        if model.remote_id.is_empty() {
            return Err("Remote model ID is required.".into());
        }
        model.display_name = self.display_name.read(cx).value().trim().to_string();
        if model.display_name.is_empty() {
            model.display_name = model.remote_id.clone();
        }
        model.capabilities = self.capabilities.clone();
        model.reasoning = self.reasoning.build(cx)?;
        model.updated_at = now_timestamp();
        Ok(model)
    }

    pub fn begin_fetch(&mut self) {
        self.fetch_status = ModelFetchStatus::Loading;
        self.available_models.clear();
    }

    pub fn finish_fetch(&mut self, models: Vec<AvailableModel>, cx: &App) {
        self.available_models = models;
        self.fetch_status = ModelFetchStatus::Loaded;
        let remote_id = self.remote_id(cx);
        self.update_capabilities_for_remote_id(&remote_id);
    }

    pub fn fail_fetch(&mut self, message: String) {
        self.available_models.clear();
        self.fetch_status = ModelFetchStatus::Failed(message);
    }

    pub fn sync_combobox(&mut self, window: &mut Window, cx: &mut Context<OneChat>) {
        let remote_id = self.remote_id(cx);
        if self.synced_models == self.available_models && self.synced_remote_id == remote_id {
            return;
        }
        self.synced_models.clone_from(&self.available_models);
        self.synced_remote_id.clone_from(&remote_id);
        let delegate = ModelIdDelegate::new(&remote_id, &self.available_models);
        self.remote_id
            .update(cx, |state, cx| state.set_items(delegate, window, cx));
    }

    pub fn select_model(
        &mut self,
        remote_id: String,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) {
        let previous_remote_id = std::mem::replace(&mut self.last_remote_id, remote_id.clone());
        let display_name = self.display_name.read(cx).value().trim().to_string();
        let (display_name, tools, vision) = synchronized_model_metadata(
            &display_name,
            &previous_remote_id,
            &remote_id,
            &self.available_models,
        );
        if let Some(display_name) = display_name {
            self.display_name
                .update(cx, |input, cx| input.set_value(display_name, window, cx));
        }
        self.capabilities.tools = tools;
        self.capabilities.vision = vision;
    }

    fn update_capabilities_for_remote_id(&mut self, remote_id: &str) {
        let (tools, vision) = model_capabilities(&self.available_models, remote_id);
        self.capabilities.tools = tools;
        self.capabilities.vision = vision;
    }

    pub fn set_capability(&mut self, capability: Capability, enabled: bool) {
        let value = match capability {
            Capability::Streaming => &mut self.capabilities.streaming,
            Capability::Tools => &mut self.capabilities.tools,
            Capability::Vision => &mut self.capabilities.vision,
        };
        *value = enabled;
    }

    pub fn capability(&self, capability: Capability) -> bool {
        match capability {
            Capability::Streaming => self.capabilities.streaming,
            Capability::Tools => self.capabilities.tools,
            Capability::Vision => self.capabilities.vision,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Capability {
    Streaming,
    Tools,
    Vision,
}

impl Capability {
    pub(in crate::desktop::ui::settings) const CORE: [Self; 3] =
        [Self::Streaming, Self::Tools, Self::Vision];

    pub(in crate::desktop::ui::settings) fn label(self) -> &'static str {
        match self {
            Self::Streaming => "Streaming",
            Self::Tools => "Tools",
            Self::Vision => "Vision",
        }
    }
}

fn synchronized_model_metadata(
    display_name: &str,
    previous_remote_id: &str,
    remote_id: &str,
    models: &[AvailableModel],
) -> (Option<String>, bool, bool) {
    let display_name = (display_name.is_empty() || display_name == previous_remote_id)
        .then(|| remote_id.to_string());
    let (tools, vision) = model_capabilities(models, remote_id);
    (display_name, tools, vision)
}

fn model_capabilities(models: &[AvailableModel], remote_id: &str) -> (bool, bool) {
    models
        .iter()
        .find(|model| model.id == remote_id.trim())
        .map(|model| (model.tools, model.vision))
        .unwrap_or_default()
}
