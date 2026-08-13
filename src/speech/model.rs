use std::ops::Range;

use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSegment {
    pub index: usize,
    pub source_range: Range<usize>,
    pub text: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthInfo {
    pub ready: bool,
    pub status: String,
    pub backend: Option<String>,
    pub configured_models: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelTask {
    Tts,
    Asr,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteModel {
    pub id: String,
    pub owned_by: Option<String>,
    pub family: Option<String>,
    pub task: ModelTask,
    pub mode: Option<String>,
    pub loaded: Option<bool>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModelCatalog {
    pub tts: Vec<RemoteModel>,
    pub asr: Vec<RemoteModel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SynthesisRequest {
    pub model: String,
    pub input: String,
    pub voice: Option<String>,
    pub seed: Option<u64>,
    pub max_tokens: Option<u32>,
    pub speed: Option<f32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub extra_options: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptionRequest {
    pub model: String,
    pub wav: Vec<u8>,
    pub language: Option<String>,
}
