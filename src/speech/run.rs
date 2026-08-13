use blake2::{Blake2s256, Digest};

use super::{audio::AudioClip, config::SpeechConfig, error::SpeechError, model::TextSegment};

#[derive(Debug, Clone, PartialEq)]
pub struct RunSnapshot {
    pub source_text: String,
    pub segments: Vec<TextSegment>,
    pub config: SpeechConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioValidationResult {
    pub ok: bool,
    pub reason: String,
    pub duration_sec: f32,
    pub rms: f32,
    pub peak: f32,
    pub active_ratio: f32,
    pub spectral_flatness: f32,
    pub zero_crossing_rate: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptValidationResult {
    pub ok: bool,
    pub expected: String,
    pub transcript: String,
    pub similarity: f32,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentStatus {
    Waiting,
    Generating,
    Validating,
    Retrying,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Planning,
    Running,
    Completed,
    Partial,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SegmentResult {
    pub segment: TextSegment,
    pub status: SegmentStatus,
    pub attempt: u32,
    pub seed: Option<u64>,
    pub clip: Option<AudioClip>,
    pub error: Option<SpeechError>,
    pub audio_validation: Option<AudioValidationResult>,
    pub transcript_validation: Option<TranscriptValidationResult>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeechRun {
    pub snapshot: RunSnapshot,
    pub status: RunStatus,
    pub segments: Vec<SegmentResult>,
    pub combined_clip: Option<AudioClip>,
    pub final_validation: Option<AudioValidationResult>,
    pub error: Option<SpeechError>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpeechEvent {
    RunStarted {
        snapshot: Box<RunSnapshot>,
    },
    SegmentChanged {
        index: usize,
        status: SegmentStatus,
        attempt: u32,
    },
    SegmentFinished {
        result: Box<SegmentResult>,
    },
    RunFinished {
        run: Box<SpeechRun>,
    },
}

pub fn derive_seed(base_seed: Option<u64>, segment_index: usize, attempt: u32) -> Option<u64> {
    let base_seed = base_seed?;
    let mut hasher = Blake2s256::new();
    hasher.update(base_seed.to_le_bytes());
    hasher.update((segment_index as u64).to_le_bytes());
    hasher.update(attempt.to_le_bytes());
    let digest = hasher.finalize();
    Some(u64::from_le_bytes(digest[..8].try_into().unwrap()) % (1_u64 << 53))
}
