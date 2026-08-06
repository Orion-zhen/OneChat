use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum SettingsSection {
    #[default]
    General,
    DefaultModels,
    SystemPrompts,
    Provider(String),
    NewProvider,
}

pub struct ProviderEditor {
    original: Option<Provider>,
    pub kind: ProviderKind,
    pub kind_menu_open: bool,
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
    pub remote_id: Entity<Composer>,
    pub display_name: Entity<Composer>,
    pub capabilities: ModelCapabilities,
    pub available_models: Vec<AvailableModel>,
    pub fetch_status: ModelFetchStatus,
    pub model_menu_open: bool,
    pub model_selection: usize,
}

impl ModelEditor {
    pub fn new(
        provider_id: String,
        provider_kind: ProviderKind,
        model: Option<Model>,
        cx: &mut Context<OneChat>,
    ) -> Self {
        let is_new = model.is_none();
        let value = model
            .clone()
            .unwrap_or_else(|| Model::new_for_provider(&provider_id, "", "", provider_kind));
        let remote_id = value.remote_id.clone();
        Self {
            original: model,
            provider_kind,
            last_remote_id: remote_id.clone(),
            provider_id,
            remote_id: cx.new(|cx| Composer::picker("Enter or select a model ID…", cx)),
            display_name: cx
                .new(|cx| Composer::single_line(value.display_name, "Display name", cx)),
            capabilities: value.capabilities,
            available_models: Vec::new(),
            fetch_status: ModelFetchStatus::Loading,
            model_menu_open: is_new,
            model_selection: 0,
        }
        .with_remote_id(remote_id, cx)
    }

    fn with_remote_id(self, remote_id: String, cx: &mut Context<OneChat>) -> Self {
        self.remote_id
            .update(cx, |input, cx| input.set_text(remote_id, cx));
        self
    }

    pub fn is_new(&self) -> bool {
        self.original.is_none()
    }

    pub(crate) fn editing_id(&self) -> Option<&str> {
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

    pub fn begin_fetch(&mut self) {
        self.fetch_status = ModelFetchStatus::Loading;
        self.available_models.clear();
        self.model_selection = 0;
        if self.is_new() {
            self.model_menu_open = true;
        }
    }

    pub fn finish_fetch(&mut self, models: Vec<AvailableModel>, cx: &App) {
        self.available_models = models;
        self.fetch_status = ModelFetchStatus::Loaded;
        self.model_selection = 0;
        if self.is_new() {
            let remote_id = self.remote_id.read(cx).text().to_string();
            self.update_vision_for_remote_id(&remote_id);
        }
    }

    pub fn fail_fetch(&mut self, message: String) {
        self.available_models.clear();
        self.fetch_status = ModelFetchStatus::Failed(message);
        self.model_selection = 0;
    }

    pub fn remote_id_changed(&mut self, remote_id: String, cx: &mut Context<OneChat>) {
        if remote_id == self.last_remote_id {
            return;
        }
        let previous_remote_id = std::mem::replace(&mut self.last_remote_id, remote_id.clone());
        let display_name = self.display_name.read(cx).text().trim().to_string();
        if display_name.is_empty() || display_name == previous_remote_id {
            self.display_name
                .update(cx, |input, cx| input.set_text(remote_id.clone(), cx));
        }
        self.update_vision_for_remote_id(&remote_id);
        self.model_selection = 0;
        self.model_menu_open = true;
    }

    pub fn toggle_model_menu(&mut self) {
        self.model_menu_open = !self.model_menu_open;
    }

    pub fn close_model_menu(&mut self) {
        self.model_menu_open = false;
    }

    pub fn navigate_models(&mut self, direction: PickerDirection, cx: &App) {
        let len = self.visible_models(cx).len();
        if len == 0 {
            self.model_selection = 0;
            return;
        }
        self.model_menu_open = true;
        self.model_selection = match direction {
            PickerDirection::Previous => self.model_selection.checked_sub(1).unwrap_or(len - 1),
            PickerDirection::Next => (self.model_selection + 1) % len,
        };
    }

    pub fn selected_model_id(&self, cx: &App) -> Option<String> {
        self.visible_models(cx)
            .get(self.model_selection)
            .map(|model| model.id.clone())
    }

    pub fn select_model(&mut self, remote_id: String, cx: &mut Context<OneChat>) {
        let previous_remote_id = std::mem::replace(&mut self.last_remote_id, remote_id.clone());
        let display_name = self.display_name.read(cx).text().trim().to_string();
        if display_name.is_empty() || display_name == previous_remote_id {
            self.display_name
                .update(cx, |input, cx| input.set_text(remote_id.clone(), cx));
        }
        self.update_vision_for_remote_id(&remote_id);
        self.remote_id
            .update(cx, |input, cx| input.set_text(remote_id, cx));
        self.model_menu_open = false;
    }

    pub fn visible_models<'a>(&'a self, cx: &App) -> Vec<&'a AvailableModel> {
        const LIMIT: usize = 100;
        let query = self.remote_id.read(cx).text().trim().to_ascii_lowercase();
        self.available_models
            .iter()
            .filter(|model| query.is_empty() || model.id.to_ascii_lowercase().contains(&query))
            .take(LIMIT)
            .collect()
    }

    fn update_vision_for_remote_id(&mut self, remote_id: &str) {
        self.capabilities.vision = self
            .available_models
            .iter()
            .find(|model| model.id == remote_id.trim())
            .is_some_and(|model| model.vision);
    }

    pub fn toggle_capability(&mut self, capability: Capability) {
        let value = match capability {
            Capability::Streaming => &mut self.capabilities.streaming,
            Capability::Vision => &mut self.capabilities.vision,
            Capability::Thinking => &mut self.capabilities.thinking,
        };
        *value = !*value;
    }

    pub fn capability(&self, capability: Capability) -> bool {
        match capability {
            Capability::Streaming => self.capabilities.streaming,
            Capability::Vision => self.capabilities.vision,
            Capability::Thinking => self.capabilities.thinking,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Capability {
    Streaming,
    Vision,
    Thinking,
}

impl Capability {
    pub(super) const CORE: [Self; 3] = [Self::Streaming, Self::Vision, Self::Thinking];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Streaming => "Streaming",
            Self::Vision => "Vision",
            Self::Thinking => "Thinking",
        }
    }
}
