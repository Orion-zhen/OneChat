use std::collections::HashSet;

use super::*;

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
    ThinkingBudget,
    Extra,
}

impl GenerationParameter {
    pub const ALL: [Self; 10] = [
        Self::Temperature,
        Self::TopP,
        Self::TopK,
        Self::MaxOutputTokens,
        Self::FrequencyPenalty,
        Self::PresencePenalty,
        Self::Seed,
        Self::StopSequences,
        Self::ThinkingBudget,
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
            Self::ThinkingBudget => "Thinking Budget",
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
            Self::ThinkingBudget => "thinking-budget",
            Self::Extra => "extra",
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
            Self::ThinkingBudget => capabilities.thinking_budget,
            Self::Extra => true,
        }
    }
}

pub struct GenerationConfigEditor {
    conversation_id: String,
    active: HashSet<GenerationParameter>,
    pub parameter_menu_open: bool,
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
        let editor = Self {
            conversation_id: conversation.id.clone(),
            active: active_parameters(config),
            parameter_menu_open: false,
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
        };
        for input in editor.inputs() {
            cx.subscribe(&input, |this, _, event, cx| {
                if matches!(
                    event,
                    crate::desktop::ui::composer::ComposerEvent::Changed(_)
                ) {
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

    pub fn is_active(&self, parameter: GenerationParameter) -> bool {
        self.active.contains(&parameter)
    }

    pub fn toggle_menu(&mut self) {
        self.parameter_menu_open = !self.parameter_menu_open;
    }

    pub fn close_menu(&mut self) {
        self.parameter_menu_open = false;
    }

    pub fn add(&mut self, parameter: GenerationParameter) {
        self.active.insert(parameter);
        self.parameter_menu_open = false;
    }

    pub fn remove(&mut self, parameter: GenerationParameter, cx: &mut Context<OneChat>) {
        self.active.remove(&parameter);
        self.parameter_menu_open = false;
        let input = self.input(parameter);
        input.update(cx, |input, cx| input.set_text("", cx));
    }

    pub fn input(&self, parameter: GenerationParameter) -> Entity<Composer> {
        match parameter {
            GenerationParameter::Temperature => self.temperature.clone(),
            GenerationParameter::TopP => self.top_p.clone(),
            GenerationParameter::TopK => self.top_k.clone(),
            GenerationParameter::MaxOutputTokens => self.max_output_tokens.clone(),
            GenerationParameter::FrequencyPenalty => self.frequency_penalty.clone(),
            GenerationParameter::PresencePenalty => self.presence_penalty.clone(),
            GenerationParameter::Seed => self.seed.clone(),
            GenerationParameter::StopSequences => self.stop_sequences.clone(),
            GenerationParameter::ThinkingBudget => self.thinking_budget.clone(),
            GenerationParameter::Extra => self.extra.clone(),
        }
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

    fn inputs(&self) -> [Entity<Composer>; 10] {
        [
            self.temperature.clone(),
            self.top_p.clone(),
            self.top_k.clone(),
            self.max_output_tokens.clone(),
            self.frequency_penalty.clone(),
            self.presence_penalty.clone(),
            self.seed.clone(),
            self.stop_sequences.clone(),
            self.thinking_budget.clone(),
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
            GenerationParameter::ThinkingBudget => config.thinking_budget.is_some(),
            GenerationParameter::Extra => !config.extra.is_empty(),
        })
        .collect()
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

    #[test]
    fn only_configured_parameters_start_active() {
        let config = GenerationConfig {
            temperature: Some(0.7),
            stop_sequences: vec!["done".into()],
            ..GenerationConfig::default()
        };
        let active = active_parameters(&config);
        assert_eq!(active.len(), 2);
        assert!(active.contains(&GenerationParameter::Temperature));
        assert!(active.contains(&GenerationParameter::StopSequences));
    }
}
