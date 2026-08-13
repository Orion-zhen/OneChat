use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReasoningEditorMode {
    #[default]
    KnownApi,
    Custom,
}

impl ReasoningEditorMode {
    pub fn index(self) -> usize {
        match self {
            Self::KnownApi => 0,
            Self::Custom => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KnownReasoningFormatItem(pub KnownReasoningFormat);

impl SearchableListItem for KnownReasoningFormatItem {
    type Value = KnownReasoningFormat;

    fn title(&self) -> SharedString {
        self.0.label().into()
    }

    fn value(&self) -> &Self::Value {
        &self.0
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        crate::desktop::ui::spaced_select_item(self.title(), cx)
    }
}

pub struct KnownReasoningPresetEditor {
    pub level: ReasoningLevel,
    pub enabled: bool,
    pub budget_tokens: Entity<InputState>,
}

pub struct CustomReasoningPresetEditor {
    pub id: Entity<InputState>,
    pub name: Entity<InputState>,
    pub request_parameters: Vec<ReasoningParameterEditor>,
    pub chat_template_kwargs: Vec<ReasoningParameterEditor>,
}

impl CustomReasoningPresetEditor {
    fn new(preset: CustomReasoningPreset, window: &mut Window, cx: &mut Context<OneChat>) -> Self {
        Self {
            id: reasoning_input(preset.id, "Preset ID", window, cx),
            name: reasoning_input(preset.name.unwrap_or_default(), "Same as ID", window, cx),
            request_parameters: preset
                .request_parameters
                .into_iter()
                .map(|parameter| {
                    ReasoningParameterEditor::new(
                        parameter,
                        ReasoningParameterScope::Request,
                        window,
                        cx,
                    )
                })
                .collect(),
            chat_template_kwargs: preset
                .chat_template_kwargs
                .into_iter()
                .map(|parameter| {
                    ReasoningParameterEditor::new(
                        parameter,
                        ReasoningParameterScope::ChatTemplateKwargs,
                        window,
                        cx,
                    )
                })
                .collect(),
        }
    }

    fn blank(index: usize, window: &mut Window, cx: &mut Context<OneChat>) -> Self {
        let id = format!("preset-{}", index + 1);
        Self::new(
            CustomReasoningPreset {
                id,
                name: None,
                request_parameters: Vec::new(),
                chat_template_kwargs: Vec::new(),
            },
            window,
            cx,
        )
    }

    fn build(&self, cx: &App) -> Result<CustomReasoningPreset, String> {
        let name = self.name.read(cx).value().trim().to_string();
        Ok(CustomReasoningPreset {
            id: self.id.read(cx).value().trim().to_string(),
            name: (!name.is_empty()).then_some(name),
            request_parameters: self
                .request_parameters
                .iter()
                .map(|parameter| parameter.build(cx))
                .collect::<Result<_, _>>()?,
            chat_template_kwargs: self
                .chat_template_kwargs
                .iter()
                .map(|parameter| parameter.build(cx))
                .collect::<Result<_, _>>()?,
        })
    }
}

pub struct ModelReasoningEditor {
    pub enabled: bool,
    pub mode: ReasoningEditorMode,
    pub format: KnownReasoningFormat,
    pub format_select: Entity<SelectState<Vec<KnownReasoningFormatItem>>>,
    pub known_default: String,
    pub known_presets: Vec<KnownReasoningPresetEditor>,
    pub custom_default: Option<usize>,
    pub custom_presets: Vec<CustomReasoningPresetEditor>,
}

impl ModelReasoningEditor {
    pub fn new(
        reasoning: Option<ModelReasoningConfig>,
        fallback_format: KnownReasoningFormat,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        let formats = KnownReasoningFormat::ALL
            .into_iter()
            .map(KnownReasoningFormatItem)
            .collect::<Vec<_>>();
        let fallback_presets = fallback_format.default_presets();
        let fallback_default = fallback_format
            .recommended_default(&fallback_presets)
            .map(|preset| preset.id().to_string())
            .unwrap_or_else(|| PROVIDER_DEFAULT_REASONING_PRESET.into());
        let (enabled, mode, format, known_default, known, custom_default_id, custom) =
            match reasoning {
                Some(ModelReasoningConfig::KnownApi {
                    format,
                    default_preset,
                    presets,
                }) => (
                    true,
                    ReasoningEditorMode::KnownApi,
                    format,
                    default_preset,
                    presets,
                    None,
                    Vec::new(),
                ),
                Some(ModelReasoningConfig::Custom {
                    default_preset,
                    presets,
                }) => (
                    true,
                    ReasoningEditorMode::Custom,
                    fallback_format,
                    fallback_default.clone(),
                    fallback_presets.clone(),
                    Some(default_preset),
                    presets,
                ),
                None => (
                    false,
                    ReasoningEditorMode::KnownApi,
                    fallback_format,
                    fallback_default,
                    fallback_presets,
                    None,
                    Vec::new(),
                ),
            };
        let format_index = formats
            .iter()
            .position(|item| item.0 == format)
            .map(IndexPath::new);
        let known_presets = known_preset_editors(format, &known, window, cx);
        let custom_presets = custom
            .into_iter()
            .map(|preset| CustomReasoningPresetEditor::new(preset, window, cx))
            .collect::<Vec<_>>();
        let custom_default = custom_default_id.and_then(|id| {
            if id == PROVIDER_DEFAULT_REASONING_PRESET {
                None
            } else {
                custom_presets
                    .iter()
                    .position(|preset| preset.id.read(cx).value().as_ref() == id)
            }
        });
        Self {
            enabled,
            mode,
            format,
            format_select: cx.new(|cx| SelectState::new(formats, format_index, window, cx)),
            known_default,
            known_presets,
            custom_default,
            custom_presets,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_mode(
        &mut self,
        mode: ReasoningEditorMode,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        if mode == ReasoningEditorMode::Custom && self.custom_presets.is_empty() {
            self.custom_presets
                .push(CustomReasoningPresetEditor::blank(0, window, cx));
            self.custom_default = Some(0);
        }
    }

    pub fn set_format(
        &mut self,
        format: KnownReasoningFormat,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) {
        if self.format == format {
            return;
        }
        self.format = format;
        let presets = format.default_presets();
        self.known_default = format
            .recommended_default(&presets)
            .map(|preset| preset.id().to_string())
            .unwrap_or_else(|| PROVIDER_DEFAULT_REASONING_PRESET.into());
        self.known_presets = known_preset_editors(format, &presets, window, cx);
    }

    pub fn toggle_known_preset(&mut self, level: ReasoningLevel, enabled: bool) {
        if let Some(preset) = self
            .known_presets
            .iter_mut()
            .find(|preset| preset.level == level)
        {
            preset.enabled = enabled;
            if !enabled && self.known_default == level.as_str() {
                self.known_default = PROVIDER_DEFAULT_REASONING_PRESET.into();
            }
        }
    }

    pub fn set_known_default(&mut self, id: String) {
        self.known_default = id;
    }

    pub fn add_custom_preset(&mut self, window: &mut Window, cx: &mut Context<OneChat>) {
        let index = self.custom_presets.len();
        self.custom_presets
            .push(CustomReasoningPresetEditor::blank(index, window, cx));
        if self.custom_default.is_none() {
            self.custom_default = Some(index);
        }
    }

    pub fn remove_custom_preset(&mut self, index: usize) {
        if index >= self.custom_presets.len() {
            return;
        }
        self.custom_presets.remove(index);
        self.custom_default = match self.custom_default {
            Some(default) if default == index => None,
            Some(default) if default > index => Some(default - 1),
            other => other,
        };
    }

    pub fn move_custom_preset(&mut self, index: usize, offset: isize) {
        let Some(target) = index.checked_add_signed(offset) else {
            return;
        };
        if index >= self.custom_presets.len() || target >= self.custom_presets.len() {
            return;
        }
        self.custom_presets.swap(index, target);
        self.custom_default = match self.custom_default {
            Some(default) if default == index => Some(target),
            Some(default) if default == target => Some(index),
            other => other,
        };
    }

    pub fn set_custom_default(&mut self, index: Option<usize>) {
        self.custom_default = index;
    }

    pub fn add_parameter(
        &mut self,
        preset: usize,
        scope: ReasoningParameterScope,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) {
        let Some(preset) = self.custom_presets.get_mut(preset) else {
            return;
        };
        let parameters = match scope {
            ReasoningParameterScope::Request => &mut preset.request_parameters,
            ReasoningParameterScope::ChatTemplateKwargs => &mut preset.chat_template_kwargs,
        };
        parameters.push(ReasoningParameterEditor::blank(scope, window, cx));
    }

    pub fn remove_parameter(
        &mut self,
        preset: usize,
        scope: ReasoningParameterScope,
        parameter: usize,
    ) {
        let Some(preset) = self.custom_presets.get_mut(preset) else {
            return;
        };
        let parameters = match scope {
            ReasoningParameterScope::Request => &mut preset.request_parameters,
            ReasoningParameterScope::ChatTemplateKwargs => &mut preset.chat_template_kwargs,
        };
        if parameter < parameters.len() {
            parameters.remove(parameter);
        }
    }

    pub fn build(&self, cx: &App) -> Result<Option<ModelReasoningConfig>, String> {
        if !self.enabled {
            return Ok(None);
        }
        let reasoning = match self.mode {
            ReasoningEditorMode::KnownApi => {
                let presets = self
                    .known_presets
                    .iter()
                    .filter(|preset| preset.enabled)
                    .map(|preset| {
                        let budget_tokens = if self.format.uses_budget()
                            && !matches!(
                                preset.level,
                                ReasoningLevel::Off | ReasoningLevel::On | ReasoningLevel::Auto
                            ) {
                            let value = preset.budget_tokens.read(cx).value();
                            Some(value.trim().parse::<i64>().map_err(|_| {
                                format!(
                                    "{} reasoning budget must be an integer.",
                                    preset.level.label()
                                )
                            })?)
                        } else {
                            None
                        };
                        Ok(KnownReasoningPreset {
                            level: preset.level,
                            budget_tokens,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                ModelReasoningConfig::KnownApi {
                    format: self.format,
                    default_preset: self.known_default.clone(),
                    presets,
                }
            }
            ReasoningEditorMode::Custom => {
                let presets = self
                    .custom_presets
                    .iter()
                    .map(|preset| preset.build(cx))
                    .collect::<Result<Vec<_>, _>>()?;
                let default_preset = self
                    .custom_default
                    .and_then(|index| presets.get(index))
                    .map(|preset| preset.id.clone())
                    .unwrap_or_else(|| PROVIDER_DEFAULT_REASONING_PRESET.into());
                ModelReasoningConfig::Custom {
                    default_preset,
                    presets,
                }
            }
        };
        reasoning.validate()?;
        Ok(Some(reasoning))
    }
}

fn known_preset_editors(
    format: KnownReasoningFormat,
    configured: &[KnownReasoningPreset],
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> Vec<KnownReasoningPresetEditor> {
    format
        .levels()
        .iter()
        .copied()
        .map(|level| {
            let configured = configured.iter().find(|preset| preset.level == level);
            let budget = configured
                .and_then(|preset| preset.budget_tokens)
                .or_else(|| level.default_budget());
            KnownReasoningPresetEditor {
                level,
                enabled: configured.is_some(),
                budget_tokens: reasoning_input(
                    budget.map(|budget| budget.to_string()).unwrap_or_default(),
                    "Token budget",
                    window,
                    cx,
                ),
            }
        })
        .collect()
}

pub(super) fn reasoning_input(
    value: impl Into<String>,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> Entity<InputState> {
    single_line_input(value, placeholder, window, cx)
}

pub fn default_reasoning_format(
    provider_kind: ProviderKind,
    remote_id: &str,
) -> KnownReasoningFormat {
    match provider_kind {
        ProviderKind::OpenAi => KnownReasoningFormat::OpenAiResponsesEffort,
        ProviderKind::Anthropic => KnownReasoningFormat::AnthropicAdaptiveEffort,
        ProviderKind::Gemini if remote_id.to_ascii_lowercase().contains("2.5") => {
            KnownReasoningFormat::GeminiThinkingBudget
        }
        ProviderKind::Gemini => KnownReasoningFormat::GeminiThinkingLevel,
        ProviderKind::OpenAiCompatible => KnownReasoningFormat::OpenAiChatEffort,
    }
}
