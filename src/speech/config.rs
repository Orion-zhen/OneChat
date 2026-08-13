use std::time::Duration;

use serde_json::{Map, Value};
use url::Url;

use super::error::SpeechError;

#[derive(Debug, Clone, PartialEq)]
pub struct SpeechConfig {
    pub endpoint: String,
    pub bearer_token: Option<String>,
    pub request_timeout: Duration,
    pub generation: GenerationConfig,
    pub segmentation: SegmentationConfig,
    pub audio_validation: AudioValidationConfig,
    pub transcript_validation: TranscriptValidationConfig,
    pub merge: MergeConfig,
    pub transport_retries: u32,
    pub transport_backoff: Duration,
    pub quality_retries: u32,
}

impl Default for SpeechConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:8080".into(),
            bearer_token: None,
            request_timeout: Duration::from_secs(600),
            generation: GenerationConfig::default(),
            segmentation: SegmentationConfig::default(),
            audio_validation: AudioValidationConfig::default(),
            transcript_validation: TranscriptValidationConfig::default(),
            merge: MergeConfig::default(),
            transport_retries: 2,
            transport_backoff: Duration::from_millis(500),
            quality_retries: 2,
        }
    }
}

impl SpeechConfig {
    pub fn normalized(mut self) -> Result<Self, SpeechError> {
        self.endpoint = normalize_endpoint(&self.endpoint)?;
        self.bearer_token = normalize_optional(self.bearer_token);
        self.generation.model = self.generation.model.trim().to_owned();
        self.generation.voice = normalize_optional(self.generation.voice);
        self.transcript_validation.model = normalize_optional(self.transcript_validation.model);
        self.transcript_validation.language =
            normalize_optional(self.transcript_validation.language);
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), SpeechError> {
        normalize_endpoint(&self.endpoint)?;
        if self.request_timeout.is_zero() {
            return Err(configuration("request timeout must be greater than zero"));
        }
        self.segmentation.validate()?;

        let generation = &self.generation;
        if generation.model.trim().is_empty() {
            return Err(configuration("a TTS model id is required"));
        }
        validate_optional_range("generation speed", generation.speed, 0.0, f32::MAX, false)?;
        validate_optional_range(
            "generation temperature",
            generation.temperature,
            0.0,
            f32::MAX,
            true,
        )?;
        validate_optional_range("generation top_p", generation.top_p, 0.0, 1.0, true)?;

        let transcript = &self.transcript_validation;
        if transcript.enabled
            && transcript
                .model
                .as_deref()
                .is_none_or(|model| model.trim().is_empty())
        {
            return Err(configuration(
                "ASR validation is enabled but no ASR model is configured",
            ));
        }
        validate_range(
            "ASR similarity threshold",
            transcript.similarity_threshold,
            0.0,
            1.0,
            true,
        )?;

        let audio = self.audio_validation;
        validate_range(
            "minimum duration",
            audio.min_duration_sec,
            0.0,
            f32::MAX,
            false,
        )?;
        validate_range("minimum RMS", audio.min_rms, 0.0, 1.0, true)?;
        for (name, value) in [
            ("minimum active ratio", audio.min_active_ratio),
            ("noise flatness", audio.noise_flatness),
            ("noise zero-crossing rate", audio.noise_zcr),
            ("noise active ratio", audio.noise_active_ratio),
        ] {
            validate_range(name, value, 0.0, 1.0, true)?;
        }
        for (name, value) in [
            ("maximum edge silence", audio.trim_max_edge_silence_sec),
            ("kept edge silence", audio.trim_keep_edge_silence_sec),
            ("merge silence", self.merge.min_silence_sec),
        ] {
            validate_range(name, value, 0.0, f32::MAX, true)?;
        }
        if audio.trim_keep_edge_silence_sec > audio.trim_max_edge_silence_sec {
            return Err(configuration(
                "kept edge silence must not exceed maximum edge silence",
            ));
        }
        Ok(())
    }
}

fn normalize_endpoint(endpoint: &str) -> Result<String, SpeechError> {
    let endpoint = endpoint.trim();
    let parsed = Url::parse(endpoint).map_err(|error| {
        configuration(format!("invalid audio.cpp endpoint {endpoint:?}: {error}"))
    })?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(configuration(
            "audio.cpp endpoint must be an absolute http(s) URL",
        ));
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(configuration(
            "audio.cpp endpoint must not contain a query string or fragment",
        ));
    }
    Ok(parsed.as_str().trim_end_matches('/').to_owned())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn validate_optional_range(
    name: &str,
    value: Option<f32>,
    min: f32,
    max: f32,
    include_min: bool,
) -> Result<(), SpeechError> {
    if let Some(value) = value {
        validate_range(name, value, min, max, include_min)?;
    }
    Ok(())
}

fn validate_range(
    name: &str,
    value: f32,
    min: f32,
    max: f32,
    include_min: bool,
) -> Result<(), SpeechError> {
    let above_min = if include_min {
        value >= min
    } else {
        value > min
    };
    if !value.is_finite() || !above_min || value > max {
        let lower = if include_min {
            "between"
        } else {
            "greater than"
        };
        let range = if include_min || max != f32::MAX {
            format!("{lower} {min} and {max}")
        } else {
            format!("{lower} {min}")
        };
        return Err(configuration(format!("{name} must be finite and {range}")));
    }
    Ok(())
}

fn configuration(message: impl Into<String>) -> SpeechError {
    SpeechError::configuration(message)
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GenerationConfig {
    pub model: String,
    pub voice: Option<String>,
    pub seed: Option<u64>,
    pub max_tokens: Option<u32>,
    pub speed: Option<f32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub extra_options: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentationConfig {
    pub min_chars: usize,
    pub target_chars: usize,
    pub max_chars: usize,
    pub spread: usize,
}

impl Default for SegmentationConfig {
    fn default() -> Self {
        Self {
            min_chars: 15,
            target_chars: 160,
            max_chars: 320,
            spread: 80,
        }
    }
}

impl SegmentationConfig {
    pub fn validate(self) -> Result<Self, SpeechError> {
        if self.min_chars == 0
            || self.min_chars > self.target_chars
            || self.target_chars > self.max_chars
            || self.spread == 0
        {
            return Err(SpeechError::configuration(
                "segmentation must satisfy 1 <= min_chars <= target_chars <= max_chars and spread > 0",
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioValidationConfig {
    pub min_duration_sec: f32,
    pub min_rms: f32,
    pub min_active_ratio: f32,
    pub noise_flatness: f32,
    pub noise_zcr: f32,
    pub noise_active_ratio: f32,
    pub trim_max_edge_silence_sec: f32,
    pub trim_keep_edge_silence_sec: f32,
}

impl Default for AudioValidationConfig {
    fn default() -> Self {
        Self {
            min_duration_sec: 0.25,
            min_rms: 0.0015,
            min_active_ratio: 0.08,
            noise_flatness: 0.55,
            noise_zcr: 0.18,
            noise_active_ratio: 0.65,
            trim_max_edge_silence_sec: 0.35,
            trim_keep_edge_silence_sec: 0.12,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptValidationConfig {
    pub enabled: bool,
    pub model: Option<String>,
    pub language: Option<String>,
    pub similarity_threshold: f32,
}

impl Default for TranscriptValidationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: None,
            language: None,
            similarity_threshold: 0.98,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MergeConfig {
    pub min_silence_sec: f32,
}

impl Default for MergeConfig {
    fn default() -> Self {
        Self {
            min_silence_sec: 0.8,
        }
    }
}
