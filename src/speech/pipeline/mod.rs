use async_channel::Sender;
use tokio_util::sync::CancellationToken;

use super::{
    backend::SpeechBackend,
    config::SpeechConfig,
    error::SpeechError,
    run::{RunSnapshot, SegmentStatus, SpeechEvent, SpeechRun},
    segmenter::{ChunkPlanner, TextSegmenter},
};
use support::{cancel_remaining, emit, emit_result, validate_run_input, waiting_results};

mod generation;
mod result;
mod support;

#[derive(Debug, Clone)]
pub struct SpeechPipeline<B, S> {
    backend: B,
    segmenter: S,
}

impl<B, S> SpeechPipeline<B, S>
where
    B: SpeechBackend,
    S: TextSegmenter + Clone,
{
    pub fn new(backend: B, segmenter: S) -> Self {
        Self { backend, segmenter }
    }

    pub async fn run(
        &self,
        text: String,
        config: SpeechConfig,
        events: &Sender<SpeechEvent>,
        cancellation: CancellationToken,
    ) -> Result<SpeechRun, SpeechError> {
        validate_run_input(&text, &config)?;
        let segments = ChunkPlanner::with_segmenter(self.segmenter.clone(), config.segmentation)?
            .plan(&text)?;
        if segments.is_empty() {
            return Err(SpeechError::segmentation(
                "sentence splitting produced no text segments",
            ));
        }
        let snapshot = RunSnapshot {
            source_text: text,
            segments,
            config,
        };
        emit(
            events,
            SpeechEvent::RunStarted {
                snapshot: Box::new(snapshot.clone()),
            },
        )
        .await;

        let mut results = waiting_results(&snapshot.segments);
        let mut cancelled = false;
        for position in 0..results.len() {
            if cancellation.is_cancelled() {
                cancelled = true;
                cancel_remaining(&mut results[position..], events).await;
                break;
            }
            let result = self
                .generate_segment(
                    &snapshot,
                    &results[position].segment,
                    1,
                    snapshot.config.quality_retries.saturating_add(1),
                    events,
                    &cancellation,
                )
                .await;
            if result.status == SegmentStatus::Cancelled {
                cancelled = true;
                results[position] = result;
                emit_result(events, &results[position]).await;
                cancel_remaining(&mut results[position + 1..], events).await;
                break;
            }
            results[position] = result;
            emit_result(events, &results[position]).await;
        }

        Ok(self.finish(snapshot, results, cancelled, events).await)
    }

    pub async fn regenerate_segment(
        &self,
        run: &SpeechRun,
        segment_index: usize,
        events: &Sender<SpeechEvent>,
        cancellation: CancellationToken,
    ) -> Result<SpeechRun, SpeechError> {
        validate_run_input(&run.snapshot.source_text, &run.snapshot.config)?;
        let position = run
            .segments
            .iter()
            .position(|result| result.segment.index == segment_index)
            .ok_or_else(|| {
                SpeechError::configuration(format!("segment {segment_index} was not found"))
            })?;
        emit(
            events,
            SpeechEvent::RunStarted {
                snapshot: Box::new(run.snapshot.clone()),
            },
        )
        .await;

        let mut results = run.segments.clone();
        let start_attempt = results[position].attempt.saturating_add(1).max(1);
        let replacement = self
            .generate_segment(
                &run.snapshot,
                &results[position].segment,
                start_attempt,
                run.snapshot.config.quality_retries.saturating_add(1),
                events,
                &cancellation,
            )
            .await;
        let cancelled = replacement.status == SegmentStatus::Cancelled;
        if !cancelled {
            results[position] = replacement;
            emit_result(events, &results[position]).await;
        }
        Ok(self
            .finish(run.snapshot.clone(), results, cancelled, events)
            .await)
    }

    pub async fn retry_failed_once(
        &self,
        run: &SpeechRun,
        events: &Sender<SpeechEvent>,
        cancellation: CancellationToken,
    ) -> Result<SpeechRun, SpeechError> {
        validate_run_input(&run.snapshot.source_text, &run.snapshot.config)?;
        emit(
            events,
            SpeechEvent::RunStarted {
                snapshot: Box::new(run.snapshot.clone()),
            },
        )
        .await;
        let mut results = run.segments.clone();
        let failed: Vec<usize> = results
            .iter()
            .enumerate()
            .filter_map(|(position, result)| {
                (result.status != SegmentStatus::Ready || result.clip.is_none()).then_some(position)
            })
            .collect();
        let mut cancelled = false;
        for position in failed {
            if cancellation.is_cancelled() {
                cancelled = true;
                break;
            }
            let start_attempt = results[position].attempt.saturating_add(1).max(1);
            let replacement = self
                .generate_segment(
                    &run.snapshot,
                    &results[position].segment,
                    start_attempt,
                    1,
                    events,
                    &cancellation,
                )
                .await;
            if replacement.status == SegmentStatus::Cancelled {
                cancelled = true;
                break;
            }
            results[position] = replacement;
            emit_result(events, &results[position]).await;
        }
        Ok(self
            .finish(run.snapshot.clone(), results, cancelled, events)
            .await)
    }
}
