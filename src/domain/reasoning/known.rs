use super::*;

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

    pub(super) fn validate_preset(self, preset: &KnownReasoningPreset) -> Result<(), String> {
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

    pub(super) fn compile(
        self,
        preset: &KnownReasoningPreset,
    ) -> Result<Map<String, Value>, String> {
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
