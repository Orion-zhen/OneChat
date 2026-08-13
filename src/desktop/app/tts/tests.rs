use std::{
    collections::VecDeque,
    f32::consts::PI,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;

use super::*;
use crate::{
    domain::AppSettings,
    speech::{
        AudioClip, HealthInfo, ModelCatalog, SentenceSpan, SpeechBackend, SpeechPipeline,
        SynthesisRequest, TextSegment, TextSegmenter, TranscriptionRequest, audio::encode_wav,
    },
};

fn snapshot() -> RunSnapshot {
    let mut config = SpeechConfig::default();
    config.generation.model = "tts".into();
    RunSnapshot {
        source_text: "hello".into(),
        segments: vec![TextSegment {
            index: 0,
            source_range: 0..5,
            text: "hello".into(),
            language: Some("en".into()),
        }],
        config,
    }
}

#[test]
fn operation_manager_rejects_conflicts_and_ignores_stale_completion() {
    let mut manager = TtsOperationManager::default();
    let (first, _) = manager.start(TtsOperationKind::Generate).unwrap();
    assert!(manager.start(TtsOperationKind::Discovery).is_none());
    assert!(!manager.finish(first + 1));
    assert!(manager.is_current(first));
    assert!(manager.finish(first));
}

#[test]
fn reducer_tracks_progress_and_stale_edits_without_mutating_settings() {
    let settings = AppSettings::default();
    let mut controller = TtsController::default();
    controller.set_source("hello".into());
    controller.apply_speech_event(SpeechEvent::RunStarted {
        snapshot: Box::new(snapshot()),
    });
    controller.apply_speech_event(SpeechEvent::SegmentChanged {
        index: 0,
        status: SegmentStatus::Validating,
        attempt: 2,
    });
    assert_eq!(controller.run.as_ref().unwrap().segments[0].attempt, 2);
    assert!(!controller.run_is_stale());
    controller.set_source("edited".into());
    assert!(controller.run_is_stale());
    assert_eq!(settings, AppSettings::default());
}

#[test]
fn regeneration_start_keeps_existing_segment_results_and_snapshot_staleness() {
    let mut controller = TtsController::default();
    controller.set_source("hello".into());
    controller.apply_speech_event(SpeechEvent::RunStarted {
        snapshot: Box::new(snapshot()),
    });
    controller.run.as_mut().unwrap().segments[0].status = SegmentStatus::Ready;
    controller.set_source("next draft".into());
    let (operation_id, _) = controller
        .operation
        .start(TtsOperationKind::Regenerate(0))
        .unwrap();
    controller.apply_speech_event(SpeechEvent::RunStarted {
        snapshot: Box::new(snapshot()),
    });
    assert_eq!(
        controller.run.as_ref().unwrap().segments[0].status,
        SegmentStatus::Ready
    );
    assert!(controller.run_is_stale());
    controller.operation.finish(operation_id);
}

#[test]
fn a_new_controller_never_restores_playground_state() {
    let mut old = TtsController::default();
    old.config.endpoint = "http://remote.test".into();
    old.config.bearer_token = Some("secret".into());
    old.config.generation.model = "tts".into();
    old.config.generation.voice = Some("voice".into());
    old.set_source("private draft".into());
    old.apply_speech_event(SpeechEvent::RunStarted {
        snapshot: Box::new(snapshot()),
    });

    let fresh = TtsController::default();
    assert_eq!(fresh.config, SpeechConfig::default());
    assert!(fresh.source.is_empty());
    assert!(fresh.run.is_none());
    assert!(fresh.discovery.catalog.tts.is_empty());
    assert!(fresh.operation.active().is_none());
}

#[derive(Clone)]
struct FixedSegmenter;

impl TextSegmenter for FixedSegmenter {
    fn sentence_spans(&self, text: &str) -> Result<Vec<SentenceSpan>, SpeechError> {
        Ok((0..text.len())
            .step_by(5)
            .map(|start| SentenceSpan {
                source_range: start..(start + 5).min(text.len()),
                language: Some("en".into()),
                paragraph: start / 5,
            })
            .collect())
    }
}

type SpeechReplies = VecDeque<Result<Vec<u8>, SpeechError>>;

#[derive(Clone)]
struct MockBackend(Arc<Mutex<SpeechReplies>>);

#[async_trait]
impl SpeechBackend for MockBackend {
    async fn health(&self) -> Result<HealthInfo, SpeechError> {
        unreachable!()
    }

    async fn models(&self) -> Result<ModelCatalog, SpeechError> {
        unreachable!()
    }

    async fn voices(&self, _: &str) -> Result<Vec<String>, SpeechError> {
        unreachable!()
    }

    async fn synthesize(&self, _: SynthesisRequest) -> Result<Vec<u8>, SpeechError> {
        self.0.lock().unwrap().pop_front().unwrap()
    }

    async fn transcribe(&self, _: TranscriptionRequest) -> Result<String, SpeechError> {
        unreachable!()
    }
}

fn valid_wav() -> Vec<u8> {
    let sample_rate = 16_000;
    let samples = (0..sample_rate / 2)
        .map(|index| (2.0 * PI * 220.0 * index as f32 / sample_rate as f32).sin() * 0.2)
        .collect::<Vec<_>>();
    encode_wav(&AudioClip::new(samples, sample_rate, 1).unwrap()).unwrap()
}

#[tokio::test]
async fn controller_reduces_a_mock_pipeline_to_a_partial_terminal_run() {
    let backend = MockBackend(Arc::new(Mutex::new(VecDeque::from([
        Ok(valid_wav()),
        Err(SpeechError::protocol("/v1/audio/speech", "bad WAV")),
    ]))));
    let pipeline = SpeechPipeline::new(backend, FixedSegmenter);
    let (sender, receiver) = async_channel::unbounded();
    let mut config = SpeechConfig::default();
    config.generation.model = "tts".into();
    config.quality_retries = 0;
    config.segmentation.min_chars = 1;
    config.segmentation.target_chars = 5;
    config.segmentation.max_chars = 5;
    config.segmentation.spread = 1;
    let run = pipeline
        .run(
            "helloworld".into(),
            config,
            &sender,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    drop(sender);

    let mut controller = TtsController::default();
    controller.set_source("helloworld".into());
    while let Ok(event) = receiver.recv().await {
        controller.apply_speech_event(event);
    }
    controller.finish_speech(Ok(run));
    let run = controller.run.unwrap();
    assert_eq!(run.status, RunStatus::Partial);
    assert_eq!(run.segments[0].status, SegmentStatus::Ready);
    assert_eq!(run.segments[1].status, SegmentStatus::Failed);
    assert!(run.combined_clip.is_some());
}

#[test]
fn reducer_accepts_all_terminal_run_states() {
    let mut controller = TtsController::default();
    for status in [
        RunStatus::Completed,
        RunStatus::Partial,
        RunStatus::Failed,
        RunStatus::Cancelled,
    ] {
        let mut run = started_run(snapshot());
        run.status = status;
        controller.apply_speech_event(SpeechEvent::RunFinished { run: Box::new(run) });
        assert_eq!(controller.run.as_ref().unwrap().status, status);
    }
}

#[test]
fn discovery_state_reduces_to_ready_and_failed_states() {
    let mut controller = TtsController::default();
    controller.begin_discovery();
    assert!(controller.discovery.loading);
    controller.apply_discovery(
        HealthInfo {
            ready: true,
            status: "ok".into(),
            backend: Some("metal".into()),
            configured_models: Some(1),
        },
        ModelCatalog::default(),
        vec!["voice".into()],
    );
    assert!(!controller.discovery.loading);
    assert_eq!(controller.discovery.voices, vec!["voice"]);

    controller.begin_discovery();
    controller.fail_discovery(SpeechError::protocol("/health", "bad response"));
    assert!(!controller.discovery.loading);
    assert!(controller.discovery.error.is_some());
}
