use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Timestamp, new_id, now_timestamp};

pub fn format_compact_token_count(tokens: u32) -> String {
    const DECIMAL_MILLION: u32 = 1_000_000;
    const BINARY_MILLION: u32 = 1_048_576;
    const DECIMAL_THOUSAND: u32 = 1_000;
    const BINARY_THOUSAND: u32 = 1_024;

    if tokens >= DECIMAL_MILLION {
        let unit = if tokens.is_multiple_of(BINARY_MILLION) {
            BINARY_MILLION
        } else {
            DECIMAL_MILLION
        };
        compact_count(tokens, unit, "M")
    } else if tokens >= DECIMAL_THOUSAND {
        let unit = if tokens.is_multiple_of(DECIMAL_THOUSAND) {
            DECIMAL_THOUSAND
        } else if tokens.is_multiple_of(BINARY_THOUSAND) {
            BINARY_THOUSAND
        } else {
            DECIMAL_THOUSAND
        };
        compact_count(tokens, unit, "K")
    } else {
        tokens.to_string()
    }
}

fn compact_count(tokens: u32, unit: u32, suffix: &str) -> String {
    if tokens.is_multiple_of(unit) {
        format!("{}{suffix}", tokens / unit)
    } else {
        let value = tokens as f64 / unit as f64;
        let formatted = format!("{value:.1}");
        format!("{}{suffix}", formatted.trim_end_matches(".0"))
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[default]
    OpenAi,
    OpenAiCompatible,
    Anthropic,
    Gemini,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "open_ai",
            Self::OpenAiCompatible => "open_ai_compatible",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::OpenAiCompatible => "OpenAI-compatible",
            Self::Anthropic => "Anthropic",
            Self::Gemini => "Gemini",
        }
    }

    pub fn default_endpoint(self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1",
            Self::OpenAiCompatible => "",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta",
        }
    }

    pub const ALL: [Self; 4] = [
        Self::OpenAi,
        Self::OpenAiCompatible,
        Self::Anthropic,
        Self::Gemini,
    ];

    pub fn default_capabilities(self) -> ModelCapabilities {
        match self {
            Self::OpenAi => ModelCapabilities {
                tools: true,
                frequency_penalty: true,
                presence_penalty: true,
                seed: true,
                ..ModelCapabilities::default()
            },
            Self::OpenAiCompatible => ModelCapabilities {
                frequency_penalty: true,
                presence_penalty: true,
                seed: true,
                ..ModelCapabilities::default()
            },
            Self::Anthropic => ModelCapabilities {
                tools: true,
                top_k: true,
                ..ModelCapabilities::default()
            },
            Self::Gemini => ModelCapabilities {
                tools: true,
                vision: true,
                top_k: true,
                frequency_penalty: true,
                presence_penalty: true,
                ..ModelCapabilities::default()
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub endpoint: String,
    pub api_key: String,
    pub headers: BTreeMap<String, String>,
    pub proxy: Option<String>,
    #[serde(default = "default_true")]
    pub streaming: bool,
    pub enabled: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

fn default_true() -> bool {
    true
}

impl Provider {
    pub fn new(name: impl Into<String>, kind: ProviderKind) -> Self {
        let now = now_timestamp();
        Self {
            id: new_id("provider"),
            name: name.into(),
            kind,
            endpoint: kind.default_endpoint().into(),
            api_key: String::new(),
            headers: BTreeMap::new(),
            proxy: None,
            streaming: true,
            enabled: true,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ModelCapabilities {
    #[serde(default)]
    pub tools: bool,
    pub vision: bool,
    #[serde(default)]
    pub audio_input: bool,
    pub temperature: bool,
    pub top_p: bool,
    pub top_k: bool,
    pub max_output_tokens: bool,
    pub frequency_penalty: bool,
    pub presence_penalty: bool,
    pub seed: bool,
    pub stop_sequences: bool,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            tools: false,
            vision: false,
            audio_input: false,
            temperature: true,
            top_p: true,
            top_k: false,
            max_output_tokens: true,
            frequency_penalty: false,
            presence_penalty: false,
            seed: false,
            stop_sequences: true,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct GenerationConfig {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub seed: Option<i64>,
    pub stop_sequences: Vec<String>,
    pub reasoning_preset: Option<String>,
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl GenerationConfig {
    pub fn filtered_for(&self, capabilities: &ModelCapabilities) -> (Self, Vec<&'static str>) {
        let mut filtered = self.clone();
        let mut ignored = Vec::new();

        macro_rules! filter_optional {
            ($capability:ident, $field:ident, $label:literal) => {
                if !capabilities.$capability && filtered.$field.take().is_some() {
                    ignored.push($label);
                }
            };
        }

        filter_optional!(temperature, temperature, "Temperature");
        filter_optional!(top_p, top_p, "Top P");
        filter_optional!(top_k, top_k, "Top K");
        filter_optional!(max_output_tokens, max_output_tokens, "Max Output");
        filter_optional!(frequency_penalty, frequency_penalty, "Frequency Penalty");
        filter_optional!(presence_penalty, presence_penalty, "Presence Penalty");
        filter_optional!(seed, seed, "Seed");
        if !capabilities.stop_sequences && !filtered.stop_sequences.is_empty() {
            filtered.stop_sequences.clear();
            ignored.push("Stop Sequences");
        }

        (filtered, ignored)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Model {
    pub id: String,
    pub provider_id: String,
    pub remote_id: String,
    pub display_name: String,
    pub capabilities: ModelCapabilities,
    #[serde(default)]
    pub context_window_tokens: Option<u32>,
    #[serde(default)]
    pub reasoning: Option<super::ModelReasoningConfig>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Model {
    pub fn new(
        provider_id: impl Into<String>,
        remote_id: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        Self::new_for_provider(provider_id, remote_id, display_name, ProviderKind::OpenAi)
    }

    pub fn new_for_provider(
        provider_id: impl Into<String>,
        remote_id: impl Into<String>,
        display_name: impl Into<String>,
        provider_kind: ProviderKind,
    ) -> Self {
        let now = now_timestamp();
        Self {
            id: new_id("model"),
            provider_id: provider_id.into(),
            remote_id: remote_id.into(),
            display_name: display_name.into(),
            capabilities: provider_kind.default_capabilities(),
            context_window_tokens: None,
            reasoning: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_support_streaming_by_default() {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        assert!(provider.streaming);

        let mut value = serde_json::to_value(provider).unwrap();
        value.as_object_mut().unwrap().remove("streaming");
        let provider: Provider = serde_json::from_value(value).unwrap();
        assert!(provider.streaming);
    }

    #[test]
    fn streaming_is_not_a_model_capability() {
        let mut value = serde_json::to_value(ModelCapabilities::default()).unwrap();
        assert!(value.get("streaming").is_none());

        value
            .as_object_mut()
            .unwrap()
            .insert("streaming".into(), true.into());
        serde_json::from_value::<ModelCapabilities>(value).unwrap();
    }

    #[test]
    fn compact_token_counts_use_common_model_units() {
        assert_eq!(format_compact_token_count(999), "999");
        assert_eq!(format_compact_token_count(128_000), "128K");
        assert_eq!(format_compact_token_count(131_072), "128K");
        assert_eq!(format_compact_token_count(1_000_000), "1M");
        assert_eq!(format_compact_token_count(1_048_576), "1M");
        assert_eq!(format_compact_token_count(1_500_000), "1.5M");
    }

    #[test]
    fn audio_capability_defaults_to_disabled() {
        let mut value = serde_json::to_value(ModelCapabilities::default()).unwrap();
        value.as_object_mut().unwrap().remove("audio_input");

        let capabilities: ModelCapabilities = serde_json::from_value(value).unwrap();
        assert!(!capabilities.audio_input);
    }
}
