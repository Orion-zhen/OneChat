use std::time::Duration;

use async_channel::Sender;
use onechat::speech::{
    AudioClip, HealthInfo, ModelCatalog, RunStatus, SegmentStatus, SegmentationConfig,
    SentenceSpan, SpeechBackend, SpeechConfig, SpeechError, SpeechErrorKind, SpeechEvent,
    SpeechPipeline, SynthesisRequest, TextSegmenter, TranscriptValidationConfig,
    TranscriptionRequest, audio::encode_wav,
};
use tokio_util::sync::CancellationToken;

use std::{
    collections::VecDeque,
    f32::consts::PI,
    future::pending,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use async_trait::async_trait;

#[derive(Clone)]
struct FixedSegmenter {
    calls: Arc<AtomicUsize>,
}

impl TextSegmenter for FixedSegmenter {
    fn sentence_spans(&self, text: &str) -> Result<Vec<SentenceSpan>, SpeechError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
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

enum Reply<T> {
    Ready(Result<T, SpeechError>),
    Pending,
}

#[derive(Default)]
struct MockState {
    speech: VecDeque<Reply<Vec<u8>>>,
    transcripts: VecDeque<Reply<String>>,
    speech_requests: Vec<SynthesisRequest>,
    transcription_requests: Vec<TranscriptionRequest>,
}

#[derive(Clone, Default)]
struct MockBackend {
    state: Arc<Mutex<MockState>>,
}

impl MockBackend {
    fn with_speech(replies: impl IntoIterator<Item = Reply<Vec<u8>>>) -> Self {
        Self {
            state: Arc::new(Mutex::new(MockState {
                speech: replies.into_iter().collect(),
                ..MockState::default()
            })),
        }
    }

    fn push_speech(&self, reply: Reply<Vec<u8>>) {
        self.state.lock().unwrap().speech.push_back(reply);
    }

    fn push_transcript(&self, reply: Reply<String>) {
        self.state.lock().unwrap().transcripts.push_back(reply);
    }

    fn speech_requests(&self) -> Vec<SynthesisRequest> {
        self.state.lock().unwrap().speech_requests.clone()
    }
}

#[async_trait]
impl SpeechBackend for MockBackend {
    async fn health(&self) -> Result<HealthInfo, SpeechError> {
        unreachable!()
    }

    async fn models(&self) -> Result<ModelCatalog, SpeechError> {
        unreachable!()
    }

    async fn voices(&self, _model: &str) -> Result<Vec<String>, SpeechError> {
        unreachable!()
    }

    async fn synthesize(&self, request: SynthesisRequest) -> Result<Vec<u8>, SpeechError> {
        let reply = {
            let mut state = self.state.lock().unwrap();
            state.speech_requests.push(request);
            state.speech.pop_front().expect("missing mock speech reply")
        };
        match reply {
            Reply::Ready(result) => result,
            Reply::Pending => pending().await,
        }
    }

    async fn transcribe(&self, request: TranscriptionRequest) -> Result<String, SpeechError> {
        let reply = {
            let mut state = self.state.lock().unwrap();
            state.transcription_requests.push(request);
            state
                .transcripts
                .pop_front()
                .expect("missing mock transcription reply")
        };
        match reply {
            Reply::Ready(result) => result,
            Reply::Pending => pending().await,
        }
    }
}

fn wav(sample_rate: u32, silent: bool) -> Vec<u8> {
    let samples = (0..sample_rate / 2)
        .map(|index| {
            if silent {
                0.0
            } else {
                (2.0 * PI * 220.0 * index as f32 / sample_rate as f32).sin() * 0.2
            }
        })
        .collect::<Vec<_>>();
    encode_wav(&AudioClip::new(samples, sample_rate, 1).unwrap()).unwrap()
}

fn config() -> SpeechConfig {
    let mut config = SpeechConfig::default();
    config.generation.model = "tts".into();
    config.generation.voice = Some("voice".into());
    config.generation.seed = Some(42);
    config.segmentation = SegmentationConfig {
        min_chars: 1,
        target_chars: 5,
        max_chars: 5,
        spread: 1,
    };
    config.transport_backoff = Duration::ZERO;
    config.quality_retries = 0;
    config
}

fn make_pipeline(
    backend: MockBackend,
    calls: Arc<AtomicUsize>,
) -> SpeechPipeline<MockBackend, FixedSegmenter> {
    SpeechPipeline::new(backend, FixedSegmenter { calls })
}

fn channels() -> (Sender<SpeechEvent>, async_channel::Receiver<SpeechEvent>) {
    async_channel::unbounded()
}

#[tokio::test]
async fn failures_do_not_interrupt_later_segments_and_events_are_ordered() {
    let backend = MockBackend::with_speech([
        Reply::Ready(Err(SpeechError::http(
            "/v1/audio/speech",
            400,
            "bad option",
        ))),
        Reply::Ready(Ok(wav(16_000, false))),
    ]);
    let calls = Arc::new(AtomicUsize::new(0));
    let pipeline = make_pipeline(backend.clone(), calls);
    let (sender, receiver) = channels();
    let run = pipeline
        .run(
            "firstsecon".into(),
            config(),
            &sender,
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert_eq!(run.status, RunStatus::Partial);
    assert_eq!(run.segments[0].status, SegmentStatus::Failed);
    assert_eq!(run.segments[1].status, SegmentStatus::Ready);
    assert!(run.combined_clip.is_some());
    assert_eq!(
        backend
            .speech_requests()
            .into_iter()
            .map(|request| request.input)
            .collect::<Vec<_>>(),
        ["first", "secon"]
    );
    let mut events = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        events.push(event);
    }
    assert!(matches!(
        events.first(),
        Some(SpeechEvent::RunStarted { .. })
    ));
    assert!(matches!(
        events.last(),
        Some(SpeechEvent::RunFinished { .. })
    ));
}

#[tokio::test]
async fn transport_retry_reuses_attempt_and_seed() {
    let backend = MockBackend::with_speech([
        Reply::Ready(Err(SpeechError::http("/v1/audio/speech", 503, "busy"))),
        Reply::Ready(Ok(wav(16_000, false))),
    ]);
    let calls = Arc::new(AtomicUsize::new(0));
    let pipeline = make_pipeline(backend.clone(), calls);
    let (sender, _) = channels();
    let run = pipeline
        .run("first".into(), config(), &sender, CancellationToken::new())
        .await
        .unwrap();
    let requests = backend.speech_requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].seed, requests[1].seed);
    assert_eq!(run.segments[0].attempt, 1);
}

#[tokio::test]
async fn quality_retry_advances_attempt_and_seed() {
    let backend = MockBackend::with_speech([
        Reply::Ready(Ok(wav(16_000, true))),
        Reply::Ready(Ok(wav(16_000, false))),
    ]);
    let calls = Arc::new(AtomicUsize::new(0));
    let pipeline = make_pipeline(backend.clone(), calls);
    let (sender, _) = channels();
    let mut settings = config();
    settings.quality_retries = 1;
    let run = pipeline
        .run("first".into(), settings, &sender, CancellationToken::new())
        .await
        .unwrap();
    let requests = backend.speech_requests();
    assert_ne!(requests[0].seed, requests[1].seed);
    assert_eq!(run.segments[0].attempt, 2);
    assert_eq!(run.segments[0].status, SegmentStatus::Ready);
}

#[tokio::test]
async fn protocol_and_permanent_errors_do_not_consume_quality_retries() {
    for error in [
        SpeechError::protocol("/v1/audio/speech", "bad WAV"),
        SpeechError::http("/v1/audio/speech", 404, "missing voice"),
    ] {
        let backend = MockBackend::with_speech([Reply::Ready(Err(error))]);
        let calls = Arc::new(AtomicUsize::new(0));
        let pipeline = make_pipeline(backend.clone(), calls);
        let (sender, _) = channels();
        let mut settings = config();
        settings.quality_retries = 3;
        let run = pipeline
            .run("first".into(), settings, &sender, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(backend.speech_requests().len(), 1);
        assert_eq!(run.status, RunStatus::Failed);
    }
}

#[tokio::test]
async fn exhausted_retryable_error_does_not_consume_quality_retries() {
    let backend = MockBackend::with_speech([
        Reply::Ready(Err(SpeechError::http("/v1/audio/speech", 429, "busy"))),
        Reply::Ready(Err(SpeechError::http("/v1/audio/speech", 429, "busy"))),
        Reply::Ready(Err(SpeechError::http("/v1/audio/speech", 429, "busy"))),
    ]);
    let pipeline = make_pipeline(backend.clone(), Arc::new(AtomicUsize::new(0)));
    let (sender, _) = channels();
    let mut settings = config();
    settings.transport_retries = 2;
    settings.quality_retries = 4;
    let run = pipeline
        .run("first".into(), settings, &sender, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(backend.speech_requests().len(), 3);
    assert_eq!(run.segments[0].attempt, 1);
    assert_eq!(run.status, RunStatus::Failed);
}

#[tokio::test]
async fn asr_mismatch_is_quality_retried_but_service_error_is_not() {
    let backend = MockBackend::with_speech([
        Reply::Ready(Ok(wav(16_000, false))),
        Reply::Ready(Ok(wav(16_000, false))),
    ]);
    backend.push_transcript(Reply::Ready(Ok("wrong".into())));
    backend.push_transcript(Reply::Ready(Ok("first".into())));
    let calls = Arc::new(AtomicUsize::new(0));
    let pipeline = make_pipeline(backend.clone(), calls);
    let (sender, _) = channels();
    let mut settings = config();
    settings.quality_retries = 1;
    settings.transcript_validation = TranscriptValidationConfig {
        enabled: true,
        model: Some("asr".into()),
        language: Some("en".into()),
        similarity_threshold: 0.98,
    };
    let run = pipeline
        .run(
            "first".into(),
            settings.clone(),
            &sender,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(run.segments[0].attempt, 2);
    assert!(run.segments[0].transcript_validation.as_ref().unwrap().ok);

    let backend = MockBackend::with_speech([Reply::Ready(Ok(wav(16_000, false)))]);
    backend.push_transcript(Reply::Ready(Err(SpeechError::protocol(
        "/v1/audio/transcriptions",
        "missing text",
    ))));
    let pipeline = make_pipeline(backend.clone(), Arc::new(AtomicUsize::new(0)));
    let run = pipeline
        .run("first".into(), settings, &sender, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(backend.speech_requests().len(), 1);
    assert_eq!(run.status, RunStatus::Failed);
}

#[tokio::test]
async fn cancellation_keeps_completed_audio_and_stops_new_requests() {
    let backend = MockBackend::with_speech([Reply::Ready(Ok(wav(16_000, false))), Reply::Pending]);
    let calls = Arc::new(AtomicUsize::new(0));
    let pipeline = make_pipeline(backend.clone(), calls);
    let (sender, _) = channels();
    let cancellation = CancellationToken::new();
    let task = tokio::spawn({
        let cancellation = cancellation.clone();
        async move {
            pipeline
                .run("firstseconthird".into(), config(), &sender, cancellation)
                .await
                .unwrap()
        }
    });
    while backend.speech_requests().len() < 2 {
        tokio::task::yield_now().await;
    }
    cancellation.cancel();
    let run = task.await.unwrap();
    assert_eq!(run.status, RunStatus::Cancelled);
    assert_eq!(run.segments[0].status, SegmentStatus::Ready);
    assert_eq!(run.segments[1].status, SegmentStatus::Cancelled);
    assert_eq!(run.segments[2].status, SegmentStatus::Cancelled);
    assert!(run.combined_clip.is_some());
    assert_eq!(backend.speech_requests().len(), 2);
}

#[tokio::test]
async fn regeneration_and_failed_retry_reuse_snapshot_without_resplitting() {
    let backend = MockBackend::with_speech([
        Reply::Ready(Err(SpeechError::http("/v1/audio/speech", 400, "bad"))),
        Reply::Ready(Ok(wav(16_000, false))),
    ]);
    let calls = Arc::new(AtomicUsize::new(0));
    let pipeline = make_pipeline(backend.clone(), calls.clone());
    let (sender, _) = channels();
    let original = pipeline
        .run(
            "firstsecon".into(),
            config(),
            &sender,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    backend.push_speech(Reply::Ready(Ok(wav(16_000, false))));
    let retried = pipeline
        .retry_failed_once(&original, &sender, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(retried.segments[0].attempt, 2);
    assert_eq!(retried.segments[1], original.segments[1]);
    assert_eq!(retried.status, RunStatus::Completed);

    backend.push_speech(Reply::Ready(Ok(wav(16_000, false))));
    let regenerated = pipeline
        .regenerate_segment(&retried, 1, &sender, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(regenerated.segments[1].attempt, 2);
    assert_eq!(regenerated.segments[0], retried.segments[0]);
}

#[tokio::test]
async fn merge_format_mismatch_is_an_explicit_run_error() {
    let backend = MockBackend::with_speech([
        Reply::Ready(Ok(wav(16_000, false))),
        Reply::Ready(Ok(wav(24_000, false))),
    ]);
    let pipeline = make_pipeline(backend, Arc::new(AtomicUsize::new(0)));
    let (sender, _) = channels();
    let run = pipeline
        .run(
            "firstsecon".into(),
            config(),
            &sender,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(run.status, RunStatus::Failed);
    assert!(run.combined_clip.is_none());
    assert_eq!(run.error.unwrap().kind, SpeechErrorKind::AudioData);
}
