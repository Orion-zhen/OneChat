use async_channel::Sender;
use tokio_util::sync::CancellationToken;

use super::{
    SpeechPipeline,
    support::{
        cancellable_sleep, cancelled_result, emit_status, failed_result, retry_delay,
        synthesis_request,
    },
};
use crate::speech::{
    audio::{decode_wav, encode_wav, trim_audio, validate_audio},
    backend::SpeechBackend,
    error::{SpeechError, SpeechErrorKind},
    model::{SynthesisRequest, TextSegment, TranscriptionRequest},
    run::{RunSnapshot, SegmentResult, SegmentStatus, SpeechEvent, derive_seed},
    segmenter::TextSegmenter,
    validation::validate_transcript,
};

impl<B, S> SpeechPipeline<B, S>
where
    B: SpeechBackend,
    S: TextSegmenter + Clone,
{
    pub(super) async fn generate_segment(
        &self,
        snapshot: &RunSnapshot,
        segment: &TextSegment,
        start_attempt: u32,
        quality_attempts: u32,
        events: &Sender<SpeechEvent>,
        cancellation: &CancellationToken,
    ) -> SegmentResult {
        let quality_attempts = quality_attempts.max(1);
        let mut last_failure = None;
        for attempt in start_attempt..start_attempt.saturating_add(quality_attempts) {
            if cancellation.is_cancelled() {
                return cancelled_result(segment.clone(), attempt);
            }
            if attempt > start_attempt {
                emit_status(events, segment.index, SegmentStatus::Retrying, attempt).await;
            }
            emit_status(events, segment.index, SegmentStatus::Generating, attempt).await;
            let seed = derive_seed(snapshot.config.generation.seed, segment.index, attempt);
            let request = synthesis_request(snapshot, segment, seed);
            let wav = match self
                .synthesize_with_retries(
                    request,
                    segment.index,
                    attempt,
                    events,
                    snapshot,
                    cancellation,
                )
                .await
            {
                Ok(wav) => wav,
                Err(error) if error.kind == SpeechErrorKind::Cancelled => {
                    return cancelled_result(segment.clone(), attempt);
                }
                Err(error) => {
                    return failed_result(segment.clone(), attempt, seed, error, None, None);
                }
            };

            emit_status(events, segment.index, SegmentStatus::Validating, attempt).await;
            let clip = match decode_wav(&wav) {
                Ok(clip) => clip,
                Err(error) => {
                    last_failure = Some(failed_result(
                        segment.clone(),
                        attempt,
                        seed,
                        SpeechError::validation(format!("WAV decode failed: {error}")),
                        None,
                        None,
                    ));
                    continue;
                }
            };
            let audio_validation = validate_audio(&clip, snapshot.config.audio_validation);
            if !audio_validation.ok {
                let error = SpeechError::validation(format!(
                    "audio validation failed: {}",
                    audio_validation.reason
                ));
                last_failure = Some(failed_result(
                    segment.clone(),
                    attempt,
                    seed,
                    error,
                    Some(audio_validation),
                    None,
                ));
                continue;
            }
            let clip = match trim_audio(&clip, snapshot.config.audio_validation) {
                Ok(clip) => clip,
                Err(error) => {
                    last_failure = Some(failed_result(
                        segment.clone(),
                        attempt,
                        seed,
                        SpeechError::validation(format!("audio trim failed: {error}")),
                        Some(audio_validation),
                        None,
                    ));
                    continue;
                }
            };

            let transcript_validation = if snapshot.config.transcript_validation.enabled {
                let wav = match encode_wav(&clip) {
                    Ok(wav) => wav,
                    Err(error) => {
                        last_failure = Some(failed_result(
                            segment.clone(),
                            attempt,
                            seed,
                            SpeechError::validation(format!(
                                "could not encode audio for ASR validation: {error}"
                            )),
                            Some(audio_validation),
                            None,
                        ));
                        continue;
                    }
                };
                let request = TranscriptionRequest {
                    model: snapshot
                        .config
                        .transcript_validation
                        .model
                        .clone()
                        .unwrap_or_default(),
                    wav,
                    language: snapshot.config.transcript_validation.language.clone(),
                };
                let transcript = match self
                    .transcribe_with_retries(
                        request,
                        segment.index,
                        attempt,
                        events,
                        snapshot,
                        cancellation,
                    )
                    .await
                {
                    Ok(transcript) => transcript,
                    Err(error) if error.kind == SpeechErrorKind::Cancelled => {
                        return cancelled_result(segment.clone(), attempt);
                    }
                    Err(error) => {
                        return failed_result(
                            segment.clone(),
                            attempt,
                            seed,
                            error,
                            Some(audio_validation),
                            None,
                        );
                    }
                };
                let validation = match validate_transcript(
                    &segment.text,
                    &transcript,
                    snapshot.config.transcript_validation.similarity_threshold,
                ) {
                    Ok(validation) => validation,
                    Err(error) => {
                        return failed_result(
                            segment.clone(),
                            attempt,
                            seed,
                            error,
                            Some(audio_validation),
                            None,
                        );
                    }
                };
                if !validation.ok {
                    last_failure = Some(failed_result(
                        segment.clone(),
                        attempt,
                        seed,
                        SpeechError::validation(format!(
                            "{}; transcript={:?}",
                            validation.reason, validation.transcript
                        )),
                        Some(audio_validation),
                        Some(validation),
                    ));
                    continue;
                }
                Some(validation)
            } else {
                None
            };

            return SegmentResult {
                segment: segment.clone(),
                status: SegmentStatus::Ready,
                attempt,
                seed,
                clip: Some(clip),
                error: None,
                audio_validation: Some(audio_validation),
                transcript_validation,
            };
        }
        last_failure.unwrap_or_else(|| {
            failed_result(
                segment.clone(),
                start_attempt,
                derive_seed(
                    snapshot.config.generation.seed,
                    segment.index,
                    start_attempt,
                ),
                SpeechError::validation("quality retry loop produced no result"),
                None,
                None,
            )
        })
    }

    async fn synthesize_with_retries(
        &self,
        request: SynthesisRequest,
        segment_index: usize,
        attempt: u32,
        events: &Sender<SpeechEvent>,
        snapshot: &RunSnapshot,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, SpeechError> {
        let mut retry = 0;
        loop {
            let result = tokio::select! {
                _ = cancellation.cancelled() => return Err(SpeechError::cancelled()),
                result = self.backend.synthesize(request.clone()) => result,
            };
            match result {
                Ok(wav) => return Ok(wav),
                Err(error) if error.retryable && retry < snapshot.config.transport_retries => {
                    retry += 1;
                    emit_status(events, segment_index, SegmentStatus::Retrying, attempt).await;
                    cancellable_sleep(
                        retry_delay(snapshot.config.transport_backoff, retry - 1),
                        cancellation,
                    )
                    .await?;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn transcribe_with_retries(
        &self,
        request: TranscriptionRequest,
        segment_index: usize,
        attempt: u32,
        events: &Sender<SpeechEvent>,
        snapshot: &RunSnapshot,
        cancellation: &CancellationToken,
    ) -> Result<String, SpeechError> {
        let mut retry = 0;
        loop {
            let result = tokio::select! {
                _ = cancellation.cancelled() => return Err(SpeechError::cancelled()),
                result = self.backend.transcribe(request.clone()) => result,
            };
            match result {
                Ok(transcript) => return Ok(transcript),
                Err(error) if error.retryable && retry < snapshot.config.transport_retries => {
                    retry += 1;
                    emit_status(events, segment_index, SegmentStatus::Retrying, attempt).await;
                    cancellable_sleep(
                        retry_delay(snapshot.config.transport_backoff, retry - 1),
                        cancellation,
                    )
                    .await?;
                }
                Err(error) => return Err(error),
            }
        }
    }
}
