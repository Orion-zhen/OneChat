use super::*;

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
