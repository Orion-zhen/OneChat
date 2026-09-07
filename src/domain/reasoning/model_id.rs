use super::*;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ModelIdReasoningConfig {
    pub default_preset: String,
    pub presets: Vec<ModelIdReasoningPreset>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ModelIdReasoningPreset {
    pub level: Option<ReasoningLevel>,
    pub model_id: String,
}

impl ModelIdReasoningPreset {
    pub fn id(&self) -> &'static str {
        self.level
            .map_or(PROVIDER_DEFAULT_REASONING_PRESET, ReasoningLevel::as_str)
    }

    pub fn label(&self) -> &'static str {
        self.level.map_or("Default", ReasoningLevel::label)
    }
}

impl ModelIdReasoningConfig {
    pub fn resolve_preset(
        &self,
        selected: Option<&str>,
    ) -> Result<&ModelIdReasoningPreset, String> {
        selected
            .and_then(|id| self.presets.iter().find(|preset| preset.id() == id))
            .or_else(|| {
                self.presets
                    .iter()
                    .find(|preset| preset.id() == self.default_preset)
            })
            .ok_or_else(|| "The default reasoning preset does not exist.".into())
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut levels = BTreeSet::new();
        for preset in &self.presets {
            if preset.model_id.trim().is_empty() {
                return Err("Reasoning model ID is required.".into());
            }
            if !levels.insert(preset.level) {
                return Err(format!(
                    "Reasoning preset {} is duplicated.",
                    preset.label()
                ));
            }
            if let Some(level) = preset.level
                && split_reasoning_model_id(&preset.model_id).map(|(_, suffix)| suffix)
                    != Some(level)
            {
                return Err(format!(
                    "Invalid model ID for the {} reasoning preset.",
                    preset.label()
                ));
            }
        }
        self.resolve_preset(None).map(|_| ())
    }
}

pub fn split_reasoning_model_id(id: &str) -> Option<(&str, ReasoningLevel)> {
    let (base, suffix) = id.rsplit_once(':')?;
    if base.is_empty() {
        return None;
    }
    let level = match suffix.to_ascii_lowercase().as_str() {
        "on" => ReasoningLevel::On,
        "off" => ReasoningLevel::Off,
        "low" => ReasoningLevel::Low,
        "medium" => ReasoningLevel::Medium,
        "high" => ReasoningLevel::High,
        "xhigh" => ReasoningLevel::Xhigh,
        "max" => ReasoningLevel::Max,
        _ => return None,
    };
    Some((base, level))
}
