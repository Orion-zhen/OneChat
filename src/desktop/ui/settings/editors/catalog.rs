use super::*;

#[derive(Clone, Debug, Default)]
pub struct ProviderFormErrors {
    pub name: Option<String>,
    pub endpoint: Option<String>,
    pub proxy: Option<String>,
    pub headers: BTreeMap<usize, String>,
}

impl ProviderFormErrors {
    fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.endpoint.is_none()
            && self.proxy.is_none()
            && self.headers.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ProviderDraft {
    kind: ProviderKind,
    name: String,
    endpoint: String,
    api_key: String,
    streaming: bool,
    headers: Vec<(String, String)>,
    proxy: String,
}

pub struct ProviderEditor {
    original: Option<Provider>,
    baseline: ProviderDraft,
    pub kind: Entity<SelectState<Vec<ProviderKindItem>>>,
    pub name: Entity<InputState>,
    pub endpoint: Entity<InputState>,
    pub api_key: Entity<InputState>,
    pub streaming: bool,
    pub headers: Vec<KeyValueEditor>,
    pub proxy: Entity<InputState>,
    pub errors: ProviderFormErrors,
    pub saving: bool,
    test_revision: u64,
    tested_draft: Option<ProviderDraft>,
    test_status: Option<ConnectionTestStatus>,
}

impl ProviderEditor {
    pub fn new(provider: Option<Provider>, window: &mut Window, cx: &mut Context<OneChat>) -> Self {
        let value = provider
            .clone()
            .unwrap_or_else(|| Provider::new("", ProviderKind::OpenAi));
        let baseline = ProviderDraft {
            kind: value.kind,
            name: value.name.clone(),
            endpoint: value.endpoint.clone(),
            api_key: value.api_key.clone(),
            streaming: value.streaming,
            headers: value
                .headers
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect(),
            proxy: value.proxy.clone().unwrap_or_default(),
        };
        let selected = ProviderKind::ALL
            .iter()
            .position(|kind| *kind == value.kind)
            .map(IndexPath::new);
        let mut headers = value
            .headers
            .into_iter()
            .map(|(name, value)| KeyValueEditor::new(name, value, window, cx))
            .collect::<Vec<_>>();
        headers.push(KeyValueEditor::new("", "", window, cx));
        Self {
            original: provider,
            baseline,
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
            streaming: value.streaming,
            headers,
            proxy: single_line_input(
                value.proxy.unwrap_or_default(),
                "Optional proxy URL",
                window,
                cx,
            ),
            errors: ProviderFormErrors::default(),
            saving: false,
            test_revision: 0,
            tested_draft: None,
            test_status: None,
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

    fn draft(&self, cx: &App) -> ProviderDraft {
        ProviderDraft {
            kind: self.kind(cx),
            name: self.name.read(cx).value().trim().to_string(),
            endpoint: self.endpoint.read(cx).value().trim().to_string(),
            api_key: self.api_key.read(cx).value().trim().to_string(),
            streaming: self.streaming,
            headers: self
                .headers
                .iter()
                .filter_map(|header| {
                    let name = header.name.read(cx).value().trim().to_string();
                    let value = header.value.read(cx).value().to_string();
                    (!name.is_empty() || !value.is_empty()).then_some((name, value))
                })
                .collect(),
            proxy: self.proxy.read(cx).value().trim().to_string(),
        }
    }

    pub fn is_dirty(&self, cx: &App) -> bool {
        self.draft(cx) != self.baseline
    }

    pub fn clear_feedback(&mut self) {
        self.errors = ProviderFormErrors::default();
        self.test_revision = self.test_revision.wrapping_add(1);
        self.tested_draft = None;
        self.test_status = None;
    }

    pub fn build(&mut self, cx: &App) -> Result<Provider, String> {
        let draft = self.draft(cx);
        self.errors = validate_provider_draft(&draft);
        if !self.errors.is_empty() {
            return Err("Fix the highlighted provider fields.".into());
        }

        let mut provider = self
            .original
            .clone()
            .unwrap_or_else(|| Provider::new("", draft.kind));
        provider.name = draft.name;
        provider.kind = draft.kind;
        provider.endpoint = draft.endpoint;
        provider.api_key = draft.api_key;
        provider.streaming = draft.streaming;
        provider.headers = draft.headers.into_iter().collect();
        provider.proxy = nonempty(&draft.proxy);
        provider.updated_at = now_timestamp();
        Ok(provider)
    }

    pub fn focus_first_error(&self, window: &mut Window, cx: &mut Context<OneChat>) {
        let input = if self.errors.name.is_some() {
            Some(self.name.clone())
        } else if self.errors.endpoint.is_some() {
            Some(self.endpoint.clone())
        } else if self.errors.proxy.is_some() {
            Some(self.proxy.clone())
        } else {
            self.errors
                .headers
                .keys()
                .next()
                .and_then(|index| self.headers.get(*index))
                .map(|header| header.name.clone())
        };
        if let Some(input) = input {
            input.update(cx, |input, cx| input.focus(window, cx));
        }
    }

    pub fn begin_test(&mut self, cx: &App) -> Result<(Provider, u64), String> {
        let provider = self.build(cx)?;
        self.test_revision = self.test_revision.wrapping_add(1);
        self.tested_draft = Some(self.draft(cx));
        self.test_status = Some(ConnectionTestStatus::Testing);
        Ok((provider, self.test_revision))
    }

    pub fn finish_test(&mut self, revision: u64, status: ConnectionTestStatus) {
        if self.test_revision == revision {
            self.test_status = Some(status);
        }
    }

    pub fn test_status(&self, cx: &App) -> Option<&ConnectionTestStatus> {
        (self.tested_draft.as_ref() == Some(&self.draft(cx)))
            .then_some(self.test_status.as_ref())
            .flatten()
    }

    pub fn add_header(&mut self, window: &mut Window, cx: &mut Context<OneChat>) {
        if self
            .headers
            .last()
            .is_some_and(|header| !header.name.read(cx).value().trim().is_empty())
        {
            self.headers.push(KeyValueEditor::new("", "", window, cx));
        }
    }

    pub fn remove_header(&mut self, index: usize) {
        if index + 1 < self.headers.len() {
            self.headers.remove(index);
        }
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
        self.clear_feedback();
    }
}

fn validate_provider_draft(draft: &ProviderDraft) -> ProviderFormErrors {
    let mut errors = ProviderFormErrors::default();
    if draft.name.is_empty() {
        errors.name = Some("Provider name is required.".into());
    }
    if draft.endpoint.is_empty() {
        if draft.kind.default_endpoint().is_empty() {
            errors.endpoint =
                Some("Endpoint is required for an OpenAI-compatible provider.".into());
        }
    } else if !valid_url(&draft.endpoint, &["http", "https"]) {
        errors.endpoint = Some("Enter a valid HTTP or HTTPS endpoint.".into());
    }
    if !draft.proxy.is_empty()
        && !valid_url(
            &draft.proxy,
            &["http", "https", "socks4", "socks4a", "socks5", "socks5h"],
        )
    {
        errors.proxy = Some("Enter a valid HTTP or SOCKS proxy URL.".into());
    }

    let mut names = BTreeSet::new();
    for (index, (name, _)) in draft.headers.iter().enumerate() {
        if name.is_empty() {
            errors
                .headers
                .insert(index, "Custom header name is required.".into());
        } else if !names.insert(name.to_ascii_lowercase()) {
            errors
                .headers
                .insert(index, format!("Custom header {name} is duplicated."));
        }
    }
    errors
}

fn valid_url(value: &str, schemes: &[&str]) -> bool {
    reqwest::Url::parse(value)
        .ok()
        .is_some_and(|url| schemes.contains(&url.scheme()) && url.host().is_some())
}

#[cfg(test)]
mod provider_tests {
    use super::*;

    fn draft() -> ProviderDraft {
        ProviderDraft {
            kind: ProviderKind::OpenAiCompatible,
            name: "Local".into(),
            endpoint: "https://example.com/v1".into(),
            api_key: String::new(),
            streaming: true,
            headers: Vec::new(),
            proxy: String::new(),
        }
    }

    #[test]
    fn validates_endpoint_and_proxy_schemes() {
        let mut value = draft();
        value.endpoint = "file:///tmp/api".into();
        value.proxy = "not a proxy".into();
        let errors = validate_provider_draft(&value);
        assert!(errors.endpoint.is_some());
        assert!(errors.proxy.is_some());
    }

    #[test]
    fn accepts_http_endpoint_and_socks_proxy() {
        let mut value = draft();
        value.proxy = "socks5h://localhost:1080".into();
        assert!(validate_provider_draft(&value).is_empty());
    }

    #[test]
    fn reports_missing_and_duplicate_header_names() {
        let mut value = draft();
        value.headers = vec![
            ("Authorization".into(), "first".into()),
            ("authorization".into(), "second".into()),
            (String::new(), "value".into()),
        ];
        let errors = validate_provider_draft(&value);
        assert_eq!(errors.headers.len(), 2);
        assert!(errors.headers.contains_key(&1));
        assert!(errors.headers.contains_key(&2));
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
            Capability::Tools => &mut self.capabilities.tools,
            Capability::Vision => &mut self.capabilities.vision,
        };
        *value = enabled;
    }

    pub fn capability(&self, capability: Capability) -> bool {
        match capability {
            Capability::Tools => self.capabilities.tools,
            Capability::Vision => self.capabilities.vision,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Capability {
    Tools,
    Vision,
}

impl Capability {
    pub(in crate::desktop::ui::settings) const CORE: [Self; 2] = [Self::Tools, Self::Vision];

    pub(in crate::desktop::ui::settings) fn label(self) -> &'static str {
        match self {
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
