use std::time::Duration;

use async_channel::Sender;
use tokio_util::sync::CancellationToken;

use crate::speech::{
    config::SpeechConfig,
    error::SpeechError,
    model::{SynthesisRequest, TextSegment},
    run::{
        AudioValidationResult, RunSnapshot, SegmentResult, SegmentStatus, SpeechEvent,
        TranscriptValidationResult,
    },
};

pub(super) fn validate_run_input(text: &str, config: &SpeechConfig) -> Result<(), SpeechError> {
    if text.trim().is_empty() {
        return Err(SpeechError::configuration(
            "speech input text must not be empty",
        ));
    }
    config.validate()
}

pub(super) fn synthesis_request(
    snapshot: &RunSnapshot,
    segment: &TextSegment,
    seed: Option<u64>,
) -> SynthesisRequest {
    let generation = &snapshot.config.generation;
    SynthesisRequest {
        model: generation.model.clone(),
        input: segment.text.clone(),
        voice: generation.voice.clone(),
        seed,
        max_tokens: generation.max_tokens,
        speed: generation.speed,
        temperature: generation.temperature,
        top_p: generation.top_p,
        extra_options: generation.extra_options.clone(),
    }
}

pub(super) fn waiting_results(segments: &[TextSegment]) -> Vec<SegmentResult> {
    segments
        .iter()
        .cloned()
        .map(|segment| SegmentResult {
            segment,
            status: SegmentStatus::Waiting,
            attempt: 0,
            seed: None,
            clip: None,
            error: None,
            audio_validation: None,
            transcript_validation: None,
        })
        .collect()
}

pub(super) fn failed_result(
    segment: TextSegment,
    attempt: u32,
    seed: Option<u64>,
    error: SpeechError,
    audio_validation: Option<AudioValidationResult>,
    transcript_validation: Option<TranscriptValidationResult>,
) -> SegmentResult {
    SegmentResult {
        segment,
        status: SegmentStatus::Failed,
        attempt,
        seed,
        clip: None,
        error: Some(error),
        audio_validation,
        transcript_validation,
    }
}

pub(super) fn cancelled_result(segment: TextSegment, attempt: u32) -> SegmentResult {
    SegmentResult {
        segment,
        status: SegmentStatus::Cancelled,
        attempt,
        seed: None,
        clip: None,
        error: Some(SpeechError::cancelled()),
        audio_validation: None,
        transcript_validation: None,
    }
}

pub(super) async fn cancel_remaining(results: &mut [SegmentResult], events: &Sender<SpeechEvent>) {
    for result in results {
        *result = cancelled_result(result.segment.clone(), result.attempt);
        emit_result(events, result).await;
    }
}

pub(super) fn retry_delay(base: Duration, exponent: u32) -> Duration {
    base.saturating_mul(2_u32.saturating_pow(exponent))
}

pub(super) async fn cancellable_sleep(
    duration: Duration,
    cancellation: &CancellationToken,
) -> Result<(), SpeechError> {
    tokio::select! {
        _ = cancellation.cancelled() => Err(SpeechError::cancelled()),
        _ = tokio::time::sleep(duration) => Ok(()),
    }
}

pub(super) async fn emit_status(
    events: &Sender<SpeechEvent>,
    index: usize,
    status: SegmentStatus,
    attempt: u32,
) {
    emit(
        events,
        SpeechEvent::SegmentChanged {
            index,
            status,
            attempt,
        },
    )
    .await;
}

pub(super) async fn emit_result(events: &Sender<SpeechEvent>, result: &SegmentResult) {
    emit(
        events,
        SpeechEvent::SegmentFinished {
            result: Box::new(result.clone()),
        },
    )
    .await;
}

pub(super) async fn emit(events: &Sender<SpeechEvent>, event: SpeechEvent) {
    let _ = events.send(event).await;
}
