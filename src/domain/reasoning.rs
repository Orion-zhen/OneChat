use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

pub const PROVIDER_DEFAULT_REASONING_PRESET: &str = "provider_default";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelReasoningConfig {
    KnownApi {
        format: KnownReasoningFormat,
        default_preset: String,
        presets: Vec<KnownReasoningPreset>,
    },
    Custom {
        default_preset: String,
        presets: Vec<CustomReasoningPreset>,
    },
}

impl ModelReasoningConfig {
    pub fn known(format: KnownReasoningFormat) -> Self {
        let presets = format.default_presets();
        let default_preset = format
            .recommended_default(&presets)
            .map(|preset| preset.id().to_string())
            .unwrap_or_else(|| PROVIDER_DEFAULT_REASONING_PRESET.into());
        Self::KnownApi {
            format,
            default_preset,
            presets,
        }
    }

    pub fn custom() -> Self {
        Self::Custom {
            default_preset: PROVIDER_DEFAULT_REASONING_PRESET.into(),
            presets: Vec::new(),
        }
    }

    pub fn default_preset(&self) -> &str {
        match self {
            Self::KnownApi { default_preset, .. } | Self::Custom { default_preset, .. } => {
                default_preset
            }
        }
    }

    pub fn preset_options(&self) -> Vec<(String, String)> {
        let mut options = vec![(PROVIDER_DEFAULT_REASONING_PRESET.into(), "Default".into())];
        match self {
            Self::KnownApi { presets, .. } => options.extend(
                presets
                    .iter()
                    .map(|preset| (preset.id().to_string(), preset.level.label().into())),
            ),
            Self::Custom { presets, .. } => options.extend(
                presets
                    .iter()
                    .map(|preset| (preset.id.clone(), preset.name().to_string())),
            ),
        }
        options
    }

    pub fn resolve_patch(
        &self,
        selected: Option<&str>,
    ) -> Result<(String, Map<String, Value>), String> {
        let requested = selected.unwrap_or_else(|| self.default_preset());
        let effective = if self.has_preset(requested) {
            requested
        } else if self.has_preset(self.default_preset()) {
            self.default_preset()
        } else {
            PROVIDER_DEFAULT_REASONING_PRESET
        };
        if effective == PROVIDER_DEFAULT_REASONING_PRESET {
            return Ok((effective.into(), Map::new()));
        }

        let patch = match self {
            Self::KnownApi {
                format, presets, ..
            } => {
                let preset = presets
                    .iter()
                    .find(|preset| preset.id() == effective)
                    .expect("effective known preset was checked");
                format.compile(preset)?
            }
            Self::Custom { presets, .. } => presets
                .iter()
                .find(|preset| preset.id == effective)
                .expect("effective custom preset was checked")
                .compile()?,
        };
        Ok((effective.into(), patch))
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::KnownApi {
                format,
                default_preset,
                presets,
            } => {
                let mut levels = BTreeSet::new();
                for preset in presets {
                    if !levels.insert(preset.level) {
                        return Err(format!(
                            "Reasoning preset {} is duplicated.",
                            preset.level.label()
                        ));
                    }
                    format.validate_preset(preset)?;
                }
                validate_default(default_preset, presets.iter().map(KnownReasoningPreset::id))
            }
            Self::Custom {
                default_preset,
                presets,
            } => {
                let mut ids = BTreeSet::new();
                for preset in presets {
                    preset.validate()?;
                    if !ids.insert(preset.id.as_str()) {
                        return Err(format!("Reasoning preset {} is duplicated.", preset.id));
                    }
                }
                validate_default(
                    default_preset,
                    presets.iter().map(|preset| preset.id.as_str()),
                )
            }
        }
    }

    fn has_preset(&self, id: &str) -> bool {
        if id == PROVIDER_DEFAULT_REASONING_PRESET {
            return true;
        }
        match self {
            Self::KnownApi { presets, .. } => presets.iter().any(|preset| preset.id() == id),
            Self::Custom { presets, .. } => presets.iter().any(|preset| preset.id == id),
        }
    }
}

fn validate_default<'a>(
    default_preset: &str,
    presets: impl Iterator<Item = &'a str>,
) -> Result<(), String> {
    if default_preset == PROVIDER_DEFAULT_REASONING_PRESET
        || presets.into_iter().any(|id| id == default_preset)
    {
        Ok(())
    } else {
        Err("The default reasoning preset does not exist.".into())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KnownReasoningFormat {
    OpenAiResponsesEffort,
    OpenAiChatEffort,
    AnthropicAdaptiveEffort,
    AnthropicManualBudget,
    GeminiThinkingLevel,
    GeminiThinkingBudget,
    DeepSeekEffort,
    QwenEffort,
    QwenThinkingBudget,
}

impl KnownReasoningFormat {
    pub const ALL: [Self; 9] = [
        Self::OpenAiResponsesEffort,
        Self::OpenAiChatEffort,
        Self::AnthropicAdaptiveEffort,
        Self::AnthropicManualBudget,
        Self::GeminiThinkingLevel,
        Self::GeminiThinkingBudget,
        Self::DeepSeekEffort,
        Self::QwenEffort,
        Self::QwenThinkingBudget,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAiResponsesEffort => "OpenAI Responses Effort",
            Self::OpenAiChatEffort => "OpenAI Chat Effort",
            Self::AnthropicAdaptiveEffort => "Anthropic Adaptive Effort",
            Self::AnthropicManualBudget => "Anthropic Manual Budget",
            Self::GeminiThinkingLevel => "Gemini Thinking Level",
            Self::GeminiThinkingBudget => "Gemini Thinking Budget",
            Self::DeepSeekEffort => "DeepSeek Effort",
            Self::QwenEffort => "Qwen Effort",
            Self::QwenThinkingBudget => "Qwen Thinking Budget",
        }
    }

    pub fn uses_budget(self) -> bool {
        matches!(
            self,
            Self::AnthropicManualBudget | Self::GeminiThinkingBudget | Self::QwenThinkingBudget
        )
    }

    pub fn levels(self) -> &'static [ReasoningLevel] {
        use ReasoningLevel::*;
        match self {
            Self::OpenAiResponsesEffort | Self::OpenAiChatEffort => {
                &[None, Minimal, Low, Medium, High, Xhigh, Max]
            }
            Self::AnthropicAdaptiveEffort => &[Off, Auto, Low, Medium, High, Xhigh, Max],
            Self::AnthropicManualBudget => &[Off, Low, Medium, High, Xhigh, Max],
            Self::GeminiThinkingLevel => &[Minimal, Low, Medium, High],
            Self::GeminiThinkingBudget => &[Off, Low, Medium, High, Xhigh, Max],
            Self::DeepSeekEffort => &[Off, Low, High, Max],
            Self::QwenEffort => &[Off, Low, Medium, High, Xhigh, Max],
            Self::QwenThinkingBudget => &[Off, On, Low, Medium, High, Xhigh, Max],
        }
    }

    pub fn recommended_default(
        self,
        presets: &[KnownReasoningPreset],
    ) -> Option<&KnownReasoningPreset> {
        [
            ReasoningLevel::Medium,
            ReasoningLevel::High,
            ReasoningLevel::Auto,
            ReasoningLevel::On,
            ReasoningLevel::Low,
            ReasoningLevel::Off,
        ]
        .into_iter()
        .find_map(|level| presets.iter().find(|preset| preset.level == level))
        .or_else(|| presets.first())
    }

    pub fn default_presets(self) -> Vec<KnownReasoningPreset> {
        self.levels()
            .iter()
            .copied()
            .filter(|level| {
                matches!(
                    level,
                    ReasoningLevel::Off
                        | ReasoningLevel::Auto
                        | ReasoningLevel::Low
                        | ReasoningLevel::Medium
                        | ReasoningLevel::High
                )
            })
            .map(|level| KnownReasoningPreset {
                level,
                budget_tokens: self.uses_budget().then(|| level.default_budget()).flatten(),
            })
            .collect()
    }

    fn validate_preset(self, preset: &KnownReasoningPreset) -> Result<(), String> {
        if !self.levels().contains(&preset.level) {
            return Err(format!(
                "{} does not support the {} reasoning preset.",
                self.label(),
                preset.level.label()
            ));
        }
        let requires_budget = self.uses_budget()
            && !matches!(
                preset.level,
                ReasoningLevel::Off | ReasoningLevel::On | ReasoningLevel::Auto
            );
        if requires_budget && preset.budget_tokens.is_none() {
            return Err(format!("{} requires a token budget.", preset.level.label()));
        }
        if preset.budget_tokens.is_some_and(|budget| budget < 0) {
            return Err(format!(
                "{} token budget must be non-negative.",
                preset.level.label()
            ));
        }
        Ok(())
    }

    fn compile(self, preset: &KnownReasoningPreset) -> Result<Map<String, Value>, String> {
        self.validate_preset(preset)?;
        let mut patch = Map::new();
        let level = preset.level;
        match self {
            Self::OpenAiResponsesEffort => {
                set_path(
                    &mut patch,
                    "reasoning.effort",
                    Value::String(level.wire_value().into()),
                )?;
            }
            Self::OpenAiChatEffort => {
                set_path(
                    &mut patch,
                    "reasoning_effort",
                    Value::String(level.wire_value().into()),
                )?;
            }
            Self::AnthropicAdaptiveEffort => match level {
                ReasoningLevel::Off => {
                    set_path(
                        &mut patch,
                        "thinking.type",
                        Value::String("disabled".into()),
                    )?;
                }
                ReasoningLevel::Auto => {
                    set_path(
                        &mut patch,
                        "thinking.type",
                        Value::String("adaptive".into()),
                    )?;
                }
                _ => {
                    set_path(
                        &mut patch,
                        "thinking.type",
                        Value::String("adaptive".into()),
                    )?;
                    set_path(
                        &mut patch,
                        "output_config.effort",
                        Value::String(level.wire_value().into()),
                    )?;
                }
            },
            Self::AnthropicManualBudget => match level {
                ReasoningLevel::Off => {
                    set_path(
                        &mut patch,
                        "thinking.type",
                        Value::String("disabled".into()),
                    )?;
                }
                _ => {
                    set_path(&mut patch, "thinking.type", Value::String("enabled".into()))?;
                    set_path(
                        &mut patch,
                        "thinking.budget_tokens",
                        Value::Number(preset.budget_tokens.expect("validated budget").into()),
                    )?;
                }
            },
            Self::GeminiThinkingLevel => {
                set_path(
                    &mut patch,
                    "generationConfig.thinkingConfig.thinkingLevel",
                    Value::String(level.wire_value().into()),
                )?;
                set_path(
                    &mut patch,
                    "generationConfig.thinkingConfig.includeThoughts",
                    Value::Bool(true),
                )?;
            }
            Self::GeminiThinkingBudget => {
                let budget = match level {
                    ReasoningLevel::Off => 0,
                    ReasoningLevel::Auto => -1,
                    _ => preset.budget_tokens.expect("validated budget"),
                };
                set_path(
                    &mut patch,
                    "generationConfig.thinkingConfig.thinkingBudget",
                    Value::Number(budget.into()),
                )?;
                set_path(
                    &mut patch,
                    "generationConfig.thinkingConfig.includeThoughts",
                    Value::Bool(true),
                )?;
            }
            Self::DeepSeekEffort => match level {
                ReasoningLevel::Off => {
                    set_path(
                        &mut patch,
                        "thinking.type",
                        Value::String("disabled".into()),
                    )?;
                }
                _ => {
                    set_path(&mut patch, "thinking.type", Value::String("enabled".into()))?;
                    set_path(
                        &mut patch,
                        "reasoning_effort",
                        Value::String(level.wire_value().into()),
                    )?;
                }
            },
            Self::QwenEffort => match level {
                ReasoningLevel::Off => {
                    set_path(&mut patch, "enable_thinking", Value::Bool(false))?;
                }
                _ => {
                    set_path(&mut patch, "enable_thinking", Value::Bool(true))?;
                    set_path(
                        &mut patch,
                        "reasoning_effort",
                        Value::String(level.wire_value().into()),
                    )?;
                }
            },
            Self::QwenThinkingBudget => match level {
                ReasoningLevel::Off => {
                    set_path(&mut patch, "enable_thinking", Value::Bool(false))?;
                }
                ReasoningLevel::On => {
                    set_path(&mut patch, "enable_thinking", Value::Bool(true))?;
                }
                _ => {
                    set_path(&mut patch, "enable_thinking", Value::Bool(true))?;
                    set_path(
                        &mut patch,
                        "thinking_budget",
                        Value::Number(preset.budget_tokens.expect("validated budget").into()),
                    )?;
                }
            },
        }
        Ok(patch)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningLevel {
    Off,
    On,
    Auto,
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl ReasoningLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::On => "on",
            Self::Auto => "auto",
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::On => "On",
            Self::Auto => "Auto",
            Self::None => "None",
            Self::Minimal => "Minimal",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Xhigh => "XHigh",
            Self::Max => "Max",
        }
    }

    fn wire_value(self) -> &'static str {
        self.as_str()
    }

    pub fn default_budget(self) -> Option<i64> {
        match self {
            Self::Low => Some(4_096),
            Self::Medium => Some(8_192),
            Self::High => Some(16_384),
            Self::Xhigh => Some(32_768),
            Self::Max => Some(65_536),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KnownReasoningPreset {
    pub level: ReasoningLevel,
    pub budget_tokens: Option<i64>,
}

impl KnownReasoningPreset {
    pub fn id(&self) -> &'static str {
        self.level.as_str()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CustomReasoningPreset {
    pub id: String,
    pub name: Option<String>,
    pub request_parameters: Vec<ReasoningParameter>,
    pub chat_template_kwargs: Vec<ReasoningParameter>,
}

impl CustomReasoningPreset {
    pub fn name(&self) -> &str {
        self.name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(&self.id)
    }

    fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("Reasoning preset ID is required.".into());
        }
        if self.id == PROVIDER_DEFAULT_REASONING_PRESET {
            return Err(format!(
                "{PROVIDER_DEFAULT_REASONING_PRESET} is reserved by OneChat."
            ));
        }
        validate_parameters(&self.request_parameters, true)?;
        validate_parameters(&self.chat_template_kwargs, false)?;
        self.compile().map(|_| ())
    }

    fn compile(&self) -> Result<Map<String, Value>, String> {
        let mut request = compile_parameters(&self.request_parameters)?;
        let kwargs = compile_parameters(&self.chat_template_kwargs)?;
        if !kwargs.is_empty() {
            set_path(&mut request, "chat_template_kwargs", Value::Object(kwargs))?;
        }
        Ok(request)
    }
}

fn validate_parameters(
    parameters: &[ReasoningParameter],
    protect_request: bool,
) -> Result<(), String> {
    let mut paths = BTreeSet::new();
    for parameter in parameters {
        let segments = path_segments(&parameter.path)?;
        if protect_request
            && matches!(
                segments[0],
                "model"
                    | "messages"
                    | "input"
                    | "contents"
                    | "system"
                    | "systemInstruction"
                    | "instructions"
                    | "stream"
                    | "tools"
                    | "tool_choice"
                    | "toolConfig"
            )
        {
            return Err(format!(
                "Reasoning parameters cannot override {}.",
                segments[0]
            ));
        }
        if !paths.insert(parameter.path.trim()) {
            return Err(format!(
                "Reasoning parameter {} is duplicated.",
                parameter.path.trim()
            ));
        }
    }
    Ok(())
}

fn compile_parameters(parameters: &[ReasoningParameter]) -> Result<Map<String, Value>, String> {
    let mut output = Map::new();
    for parameter in parameters {
        set_path(&mut output, &parameter.path, parameter.value.to_json()?)?;
    }
    Ok(output)
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ReasoningParameter {
    pub path: String,
    pub value: ReasoningParameterValue,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ReasoningParameterValue {
    String(String),
    Integer(i64),
    Decimal(f64),
    Boolean(bool),
    Null,
}

impl ReasoningParameterValue {
    fn to_json(&self) -> Result<Value, String> {
        match self {
            Self::String(value) => Ok(Value::String(value.clone())),
            Self::Integer(value) => Ok(Value::Number((*value).into())),
            Self::Decimal(value) => Number::from_f64(*value)
                .map(Value::Number)
                .ok_or_else(|| "Decimal reasoning parameters must be finite.".into()),
            Self::Boolean(value) => Ok(Value::Bool(*value)),
            Self::Null => Ok(Value::Null),
        }
    }
}

fn path_segments(path: &str) -> Result<Vec<&str>, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Reasoning parameter path is required.".into());
    }
    let segments = path.split('.').map(str::trim).collect::<Vec<_>>();
    if segments.iter().any(|segment| segment.trim().is_empty()) {
        return Err(format!(
            "Reasoning parameter path {path} contains an empty segment."
        ));
    }
    Ok(segments)
}

fn set_path(target: &mut Map<String, Value>, path: &str, value: Value) -> Result<(), String> {
    let segments = path_segments(path)?;
    let mut current = target;
    for segment in &segments[..segments.len() - 1] {
        let entry = current
            .entry((*segment).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        let Value::Object(object) = entry else {
            return Err(format!(
                "Reasoning parameter path {path} conflicts with another parameter."
            ));
        };
        current = object;
    }
    let leaf = segments[segments.len() - 1];
    if current.insert(leaf.into(), value).is_some() {
        return Err(format!("Reasoning parameter path {path} is duplicated."));
    }
    Ok(())
}

pub fn merge_json_patch(target: &mut Map<String, Value>, patch: Map<String, Value>) {
    for (key, patch_value) in patch {
        if patch_value.is_null() {
            target.remove(&key);
            continue;
        }
        match (target.get_mut(&key), patch_value) {
            (Some(Value::Object(target_object)), Value::Object(patch_object)) => {
                merge_json_patch(target_object, patch_object);
            }
            (_, value) => {
                target.insert(key, value);
            }
        }
    }
}
