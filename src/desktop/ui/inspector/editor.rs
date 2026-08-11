use std::collections::HashSet;

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReasoningPresetItem {
    value: Option<String>,
    label: SharedString,
}

impl SearchableListItem for ReasoningPresetItem {
    type Value = Option<String>;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        crate::desktop::ui::spaced_select_item(self.title(), cx)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GenerationParameter {
    Temperature,
    TopP,
    TopK,
    MaxOutputTokens,
    FrequencyPenalty,
    PresencePenalty,
    Seed,
    StopSequences,
    Extra,
}

impl GenerationParameter {
    pub const ALL: [Self; 9] = [
        Self::Temperature,
        Self::TopP,
        Self::TopK,
        Self::MaxOutputTokens,
        Self::FrequencyPenalty,
        Self::PresencePenalty,
        Self::Seed,
        Self::StopSequences,
        Self::Extra,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Temperature => "Temperature",
            Self::TopP => "Top P",
            Self::TopK => "Top K",
            Self::MaxOutputTokens => "Max Output",
            Self::FrequencyPenalty => "Frequency Penalty",
            Self::PresencePenalty => "Presence Penalty",
            Self::Seed => "Seed",
            Self::StopSequences => "Stop Sequences",
            Self::Extra => "Provider-specific Parameters",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Temperature => "temperature",
            Self::TopP => "top-p",
            Self::TopK => "top-k",
            Self::MaxOutputTokens => "max-output-tokens",
            Self::FrequencyPenalty => "frequency-penalty",
            Self::PresencePenalty => "presence-penalty",
            Self::Seed => "seed",
            Self::StopSequences => "stop-sequences",
            Self::Extra => "extra",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Temperature => "Controls randomness",
            Self::TopP => "Limits probability mass",
            Self::TopK => "Limits candidate tokens",
            Self::MaxOutputTokens => "Maximum response length",
            Self::FrequencyPenalty => "Reduces repeated tokens",
            Self::PresencePenalty => "Encourages new topics",
            Self::Seed => "Makes output repeatable",
            Self::StopSequences => "One sequence per line",
            Self::Extra => "Provider-specific JSON",
        }
    }

    pub fn supported_by(self, capabilities: &crate::domain::ModelCapabilities) -> bool {
        match self {
            Self::Temperature => capabilities.temperature,
            Self::TopP => capabilities.top_p,
            Self::TopK => capabilities.top_k,
            Self::MaxOutputTokens => capabilities.max_output_tokens,
            Self::FrequencyPenalty => capabilities.frequency_penalty,
            Self::PresencePenalty => capabilities.presence_penalty,
            Self::Seed => capabilities.seed,
            Self::StopSequences => capabilities.stop_sequences,
            Self::Extra => true,
        }
    }

    pub fn is_multiline(self) -> bool {
        matches!(self, Self::StopSequences | Self::Extra)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationParameterItem(GenerationParameter);

impl SearchableListItem for GenerationParameterItem {
    type Value = GenerationParameter;

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

pub struct GenerationConfigEditor {
    conversation_id: String,
    active: HashSet<GenerationParameter>,
    pub parameter_select: Entity<SelectState<Vec<GenerationParameterItem>>>,
    synced_options: Vec<GenerationParameterItem>,
    reasoning_preset: Option<String>,
    synced_reasoning_options: Vec<ReasoningPresetItem>,
    pub reasoning_select: Entity<SelectState<Vec<ReasoningPresetItem>>>,
    pub temperature: Entity<InputState>,
    pub top_p: Entity<InputState>,
    pub top_k: Entity<InputState>,
    pub max_output_tokens: Entity<InputState>,
    pub frequency_penalty: Entity<InputState>,
    pub presence_penalty: Entity<InputState>,
    pub seed: Entity<InputState>,
    pub stop_sequences: Entity<InputState>,
    pub extra: Entity<InputState>,
}

impl GenerationConfigEditor {
    pub fn new(
        conversation: &Conversation,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        let config = &conversation.generation_config;
        let editor = Self {
            conversation_id: conversation.id.clone(),
            active: active_parameters(config),
            parameter_select: cx.new(|cx| SelectState::new(Vec::new(), None, window, cx)),
            synced_options: Vec::new(),
            reasoning_preset: config.reasoning_preset.clone(),
            synced_reasoning_options: Vec::new(),
            reasoning_select: cx.new(|cx| SelectState::new(Vec::new(), None, window, cx)),
            temperature: optional_number_input(config.temperature, None, window, cx),
            top_p: optional_number_input(config.top_p, None, window, cx),
            top_k: optional_number_input(config.top_k, Some(0.0), window, cx),
            max_output_tokens: optional_number_input(
                config.max_output_tokens,
                Some(0.0),
                window,
                cx,
            ),
            frequency_penalty: optional_number_input(config.frequency_penalty, None, window, cx),
            presence_penalty: optional_number_input(config.presence_penalty, None, window, cx),
            seed: optional_number_input(config.seed, None, window, cx),
            stop_sequences: multiline_input(
                config.stop_sequences.join("\n"),
                "One stop sequence per line",
                window,
                cx,
            ),
            extra: multiline_input(
                serde_json::to_string_pretty(&config.extra).unwrap_or_else(|_| "{}".into()),
                "Provider-specific JSON object",
                window,
                cx,
            ),
        };
        for input in editor.inputs() {
            cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.schedule_generation_config_save(cx);
                }
            })
            .detach();
        }
        editor
    }

    pub fn is_for(&self, conversation_id: &str) -> bool {
        self.conversation_id == conversation_id
    }

    pub fn reasoning_preset(&self) -> Option<&str> {
        self.reasoning_preset.as_deref()
    }

    pub fn set_reasoning_preset(&mut self, preset: Option<String>) {
        self.reasoning_preset = preset;
    }

    pub fn sync_reasoning_select(
        &mut self,
        model: &Model,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) {
        let Some(reasoning) = &model.reasoning else {
            if !self.synced_reasoning_options.is_empty() {
                self.synced_reasoning_options.clear();
                self.reasoning_select.update(cx, |select, cx| {
                    select.set_items(Vec::new(), window, cx);
                    select.set_selected_index(None, window, cx);
                });
            }
            return;
        };
        let options = reasoning
            .preset_options()
            .into_iter()
            .map(|(id, label)| ReasoningPresetItem {
                value: Some(id),
                label: label.into(),
            })
            .collect::<Vec<_>>();
        let desired = Some(
            self.reasoning_preset
                .clone()
                .filter(|id| options.iter().any(|item| item.value.as_ref() == Some(id)))
                .unwrap_or_else(|| reasoning.default_preset().to_string()),
        );
        let changed = options != self.synced_reasoning_options;
        if changed {
            self.synced_reasoning_options.clone_from(&options);
        }
        if changed
            || self.reasoning_select.read(cx).selected_value().cloned() != Some(desired.clone())
        {
            self.reasoning_select.update(cx, |select, cx| {
                if changed {
                    select.set_items(options, window, cx);
                }
                select.set_selected_value(&desired, window, cx);
            });
        }
    }

    pub fn is_active(&self, parameter: GenerationParameter) -> bool {
        self.active.contains(&parameter)
    }

    pub fn add(&mut self, parameter: GenerationParameter) {
        self.active.insert(parameter);
    }

    pub fn remove(
        &mut self,
        parameter: GenerationParameter,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) {
        self.active.remove(&parameter);
        self.input(parameter)
            .update(cx, |input, cx| input.set_value("", window, cx));
    }

    pub fn input(&self, parameter: GenerationParameter) -> Entity<InputState> {
        match parameter {
            GenerationParameter::Temperature => self.temperature.clone(),
            GenerationParameter::TopP => self.top_p.clone(),
            GenerationParameter::TopK => self.top_k.clone(),
            GenerationParameter::MaxOutputTokens => self.max_output_tokens.clone(),
            GenerationParameter::FrequencyPenalty => self.frequency_penalty.clone(),
            GenerationParameter::PresencePenalty => self.presence_penalty.clone(),
            GenerationParameter::Seed => self.seed.clone(),
            GenerationParameter::StopSequences => self.stop_sequences.clone(),
            GenerationParameter::Extra => self.extra.clone(),
        }
    }

    pub fn sync_parameter_select(
        &mut self,
        capabilities: &crate::domain::ModelCapabilities,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) {
        let options = GenerationParameter::ALL
            .into_iter()
            .filter(|parameter| parameter.supported_by(capabilities) && !self.is_active(*parameter))
            .map(GenerationParameterItem)
            .collect::<Vec<_>>();
        if options == self.synced_options {
            return;
        }
        self.synced_options.clone_from(&options);
        self.parameter_select.update(cx, |select, cx| {
            select.set_items(options, window, cx);
            select.set_selected_index(None, window, cx);
        });
    }

    pub fn build(&self, base: &GenerationConfig, cx: &App) -> Result<GenerationConfig, String> {
        let mut config = base.clone();
        config.reasoning_preset.clone_from(&self.reasoning_preset);
        config.temperature =
            parse_optional_f64("Temperature", self.temperature.read(cx).value().as_ref())?;
        config.top_p = parse_optional_f64("Top P", self.top_p.read(cx).value().as_ref())?;
        config.top_k = parse_optional("Top K", self.top_k.read(cx).value().as_ref())?;
        config.max_output_tokens = parse_optional(
            "Max Output",
            self.max_output_tokens.read(cx).value().as_ref(),
        )?;
        config.frequency_penalty = parse_optional_f64(
            "Frequency Penalty",
            self.frequency_penalty.read(cx).value().as_ref(),
        )?;
        config.presence_penalty = parse_optional_f64(
            "Presence Penalty",
            self.presence_penalty.read(cx).value().as_ref(),
        )?;
        config.seed = parse_optional("Seed", self.seed.read(cx).value().as_ref())?;
        config.stop_sequences = self
            .stop_sequences
            .read(cx)
            .value()
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect();
        config.extra = parse_json_object(self.extra.read(cx).value().as_ref())?;
        Ok(config)
    }

    fn inputs(&self) -> [Entity<InputState>; 9] {
        [
            self.temperature.clone(),
            self.top_p.clone(),
            self.top_k.clone(),
            self.max_output_tokens.clone(),
            self.frequency_penalty.clone(),
            self.presence_penalty.clone(),
            self.seed.clone(),
            self.stop_sequences.clone(),
            self.extra.clone(),
        ]
    }
}

fn active_parameters(config: &GenerationConfig) -> HashSet<GenerationParameter> {
    GenerationParameter::ALL
        .into_iter()
        .filter(|parameter| match parameter {
            GenerationParameter::Temperature => config.temperature.is_some(),
            GenerationParameter::TopP => config.top_p.is_some(),
            GenerationParameter::TopK => config.top_k.is_some(),
            GenerationParameter::MaxOutputTokens => config.max_output_tokens.is_some(),
            GenerationParameter::FrequencyPenalty => config.frequency_penalty.is_some(),
            GenerationParameter::PresencePenalty => config.presence_penalty.is_some(),
            GenerationParameter::Seed => config.seed.is_some(),
            GenerationParameter::StopSequences => !config.stop_sequences.is_empty(),
            GenerationParameter::Extra => !config.extra.is_empty(),
        })
        .collect()
}

fn optional_number_input<T: Display>(
    value: Option<T>,
    min: Option<f64>,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> Entity<InputState> {
    cx.new(|cx| {
        let mut input = InputState::new(window, cx)
            .default_value(value.map(|value| value.to_string()).unwrap_or_default())
            .placeholder("")
            .mask_pattern(MaskPattern::Number {
                separator: None,
                fraction: None,
            });
        if let Some(min) = min {
            input = input.min(min);
        }
        input
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
