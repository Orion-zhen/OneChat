use super::*;

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
    pub context_window: Entity<InputState>,
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
        let context_window = value
            .context_window_tokens
            .map(|tokens| tokens.to_string())
            .unwrap_or_default();
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
            context_window: single_line_input(context_window, "Unknown or token count", window, cx),
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
        model.context_window_tokens =
            parse_context_window_tokens(self.context_window.read(cx).value().as_ref())?;
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
        let models_changed = self.synced_models != self.available_models;
        if !models_changed && self.synced_remote_id == remote_id {
            return;
        }
        if models_changed {
            let current = self.context_window.read(cx).value().to_string();
            if let Some(tokens) =
                context_window_to_sync(&self.available_models, &remote_id, &current)
            {
                self.context_window.update(cx, |input, cx| {
                    input.set_value(tokens.to_string(), window, cx)
                });
            }
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
        let remote_id_changed = previous_remote_id.trim() != remote_id.trim();
        let display_name = self.display_name.read(cx).value().trim().to_string();
        let synchronized = synchronized_model_metadata(
            &display_name,
            &previous_remote_id,
            &remote_id,
            &self.available_models,
        );
        if let Some(display_name) = synchronized.display_name {
            self.display_name
                .update(cx, |input, cx| input.set_value(display_name, window, cx));
        }
        self.capabilities.vision = synchronized.metadata.vision;
        self.capabilities.audio = synchronized.metadata.audio;
        self.capabilities.tools = synchronized.metadata.tools;
        if remote_id_changed {
            let context_window = synchronized
                .metadata
                .context_window_tokens
                .map(|tokens| tokens.to_string())
                .unwrap_or_default();
            self.context_window
                .update(cx, |input, cx| input.set_value(context_window, window, cx));
        }
    }

    fn update_capabilities_for_remote_id(&mut self, remote_id: &str) {
        let metadata = discovered_model_metadata(&self.available_models, remote_id);
        self.capabilities.vision = metadata.vision;
        self.capabilities.audio = metadata.audio;
        self.capabilities.tools = metadata.tools;
    }

    pub fn set_capability(&mut self, capability: Capability, enabled: bool) {
        let value = match capability {
            Capability::Vision => &mut self.capabilities.vision,
            Capability::Audio => &mut self.capabilities.audio,
            Capability::Tools => &mut self.capabilities.tools,
        };
        *value = enabled;
    }

    pub fn capability(&self, capability: Capability) -> bool {
        match capability {
            Capability::Vision => self.capabilities.vision,
            Capability::Audio => self.capabilities.audio,
            Capability::Tools => self.capabilities.tools,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Capability {
    Vision,
    Audio,
    Tools,
}

impl Capability {
    pub(in crate::desktop::ui::settings) const CORE: [Self; 3] =
        [Self::Vision, Self::Audio, Self::Tools];

    pub(in crate::desktop::ui::settings) fn label(self) -> &'static str {
        match self {
            Self::Vision => "Vision",
            Self::Audio => "Audio",
            Self::Tools => "Tools",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DiscoveredModelMetadata {
    vision: bool,
    audio: bool,
    tools: bool,
    context_window_tokens: Option<u32>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SynchronizedModelMetadata {
    display_name: Option<String>,
    metadata: DiscoveredModelMetadata,
}

fn synchronized_model_metadata(
    display_name: &str,
    previous_remote_id: &str,
    remote_id: &str,
    models: &[AvailableModel],
) -> SynchronizedModelMetadata {
    SynchronizedModelMetadata {
        display_name: (display_name.is_empty() || display_name == previous_remote_id)
            .then(|| remote_id.to_string()),
        metadata: discovered_model_metadata(models, remote_id),
    }
}

fn discovered_model_metadata(
    models: &[AvailableModel],
    remote_id: &str,
) -> DiscoveredModelMetadata {
    models
        .iter()
        .find(|model| model.id == remote_id.trim())
        .map(|model| DiscoveredModelMetadata {
            vision: model.vision,
            audio: model.audio,
            tools: model.tools,
            context_window_tokens: model.context_window_tokens,
        })
        .unwrap_or_default()
}

fn context_window_to_sync(
    models: &[AvailableModel],
    remote_id: &str,
    current: &str,
) -> Option<u32> {
    current
        .trim()
        .is_empty()
        .then(|| discovered_model_metadata(models, remote_id).context_window_tokens)
        .flatten()
}

fn parse_context_window_tokens(value: &str) -> Result<Option<u32>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    value
        .parse::<u32>()
        .ok()
        .filter(|tokens| *tokens > 0)
        .map(Some)
        .ok_or_else(|| "Context Window must be a positive whole number up to 4,294,967,295.".into())
}

#[cfg(test)]
mod model_tests {
    use super::*;

    fn available_model(id: &str, context_window_tokens: Option<u32>) -> AvailableModel {
        AvailableModel {
            id: id.into(),
            tools: true,
            vision: true,
            audio: false,
            context_window_tokens,
        }
    }

    #[test]
    fn synchronizes_discovered_context_window_without_overwriting_manual_value() {
        let models = [available_model("known", Some(32_768))];

        assert_eq!(context_window_to_sync(&models, "known", ""), Some(32_768));
        assert_eq!(context_window_to_sync(&models, "known", "65536"), None);
        assert_eq!(context_window_to_sync(&models, "custom", ""), None);
    }

    #[test]
    fn validates_optional_context_window_tokens() {
        assert_eq!(parse_context_window_tokens(""), Ok(None));
        assert_eq!(parse_context_window_tokens(" 128000 "), Ok(Some(128_000)));
        for invalid in ["0", "-1", "1.5", "4294967296", "many"] {
            assert!(parse_context_window_tokens(invalid).is_err());
        }
    }

    #[test]
    fn synchronizes_named_discovered_metadata_and_clears_custom_ids() {
        let models = [available_model("known", Some(128_000))];
        let synchronized = synchronized_model_metadata("old", "old", "known", &models);
        assert_eq!(synchronized.display_name.as_deref(), Some("known"));
        assert_eq!(
            synchronized.metadata,
            DiscoveredModelMetadata {
                vision: true,
                audio: false,
                tools: true,
                context_window_tokens: Some(128_000),
            }
        );

        let custom = synchronized_model_metadata("Known", "known", "custom", &models);
        assert_eq!(custom.display_name, None);
        assert_eq!(custom.metadata, DiscoveredModelMetadata::default());
    }
}
