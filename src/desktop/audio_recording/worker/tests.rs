use super::super::{AudioRecording, RECORDING_SAMPLE_RATE, RecordingSnapshot};
use super::*;
use crate::desktop::audio_recording::capture::ActiveInput;

struct FakeInput;
impl ActiveInput for FakeInput {}

struct FakeBackend {
    samples: Vec<Vec<f32>>,
    error: Option<String>,
    start_error: Option<String>,
}

struct PanickingBackend;

impl RecordingBackend for PanickingBackend {
    fn start(&mut self) -> Result<InputSession, String> {
        panic!("native audio backend initialization failed");
    }
}

impl RecordingBackend for FakeBackend {
    fn start(&mut self) -> Result<InputSession, String> {
        if let Some(error) = self.start_error.take() {
            return Err(error);
        }
        let (sample_tx, samples) = mpsc::sync_channel(8);
        for samples in std::mem::take(&mut self.samples) {
            sample_tx.send(samples).unwrap();
        }
        let (error_tx, errors) = mpsc::channel();
        if let Some(error) = self.error.take() {
            error_tx.send(error).unwrap();
        }
        Ok(InputSession {
            sample_rate: RECORDING_SAMPLE_RATE,
            samples,
            errors,
            _stream: Box::new(FakeInput),
        })
    }
}

fn wait_for(recorder: &AudioRecording, predicate: impl Fn(&RecordingSnapshot) -> bool) {
    for _ in 0..100 {
        if predicate(&recorder.snapshot()) {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("recording state did not update: {:?}", recorder.snapshot());
}

#[test]
fn recorder_finalizes_and_can_be_cancelled_without_a_device() {
    let recorder = AudioRecording::with_backend(Box::new(FakeBackend {
        samples: vec![vec![0.25; 1_600]],
        error: None,
        start_error: None,
    }));
    recorder.start();
    wait_for(&recorder, |state| {
        state.status == RecordingStatus::Recording
    });
    recorder.start();
    assert_eq!(recorder.snapshot().status, RecordingStatus::Recording);
    recorder.stop();
    wait_for(&recorder, |state| {
        state.status == RecordingStatus::Completed
    });
    let output = recorder.snapshot().output.unwrap();
    assert_eq!(output.duration_ms, 100);
    assert!(output.wav.starts_with(b"RIFF"));

    recorder.reset();
    wait_for(&recorder, |state| state.status == RecordingStatus::Idle);

    let cancelled = AudioRecording::with_backend(Box::new(FakeBackend {
        samples: vec![vec![0.25; 1_600]],
        error: None,
        start_error: None,
    }));
    cancelled.start();
    wait_for(&cancelled, |state| {
        state.status == RecordingStatus::Recording
    });
    cancelled.cancel();
    wait_for(&cancelled, |state| state.status == RecordingStatus::Idle);
    assert!(cancelled.snapshot().output.is_none());
}

#[test]
fn recorder_surfaces_start_and_stream_failures() {
    let start_failure = AudioRecording::with_backend(Box::new(FakeBackend {
        samples: Vec::new(),
        error: None,
        start_error: Some("Microphone access was denied.".into()),
    }));
    start_failure.start();
    wait_for(&start_failure, |state| {
        state.status == RecordingStatus::Failed
    });
    assert_eq!(
        start_failure.snapshot().error.as_deref(),
        Some("Microphone access was denied.")
    );

    let stream_failure = AudioRecording::with_backend(Box::new(FakeBackend {
        samples: Vec::new(),
        error: Some("Microphone disconnected.".into()),
        start_error: None,
    }));
    stream_failure.start();
    wait_for(&stream_failure, |state| {
        state.status == RecordingStatus::Failed
    });
    assert_eq!(
        stream_failure.snapshot().error.as_deref(),
        Some("Microphone disconnected.")
    );

    let backend_panic = AudioRecording::with_backend(Box::new(PanickingBackend));
    backend_panic.start();
    wait_for(&backend_panic, |state| {
        state.status == RecordingStatus::Failed
    });
    assert_eq!(
        backend_panic.snapshot().error.as_deref(),
        Some("Could not initialize microphone capture.")
    );
}
