use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
};

mod capture;
mod pipeline;
mod worker;

use capture::{CpalBackend, RecordingBackend};
use worker::Command;

pub(super) const RECORDING_SAMPLE_RATE: u32 = 16_000;
pub(super) const MAX_RECORDING_DURATION_MS: u64 = 5 * 60 * 1_000;
pub(crate) const RECORDING_WAVEFORM_SAMPLES: usize = 96;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum RecordingStatus {
    #[default]
    Idle,
    RequestingPermission,
    Recording,
    Finalizing,
    Completed,
    Failed,
}

impl RecordingStatus {
    pub(crate) fn is_active(self) -> bool {
        matches!(
            self,
            Self::RequestingPermission | Self::Recording | Self::Finalizing
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecordingLimit {
    Duration,
    Size,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecordingOutput {
    pub(crate) wav: Vec<u8>,
    pub(crate) duration_ms: u64,
    pub(crate) limit: Option<RecordingLimit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecordingSnapshot {
    pub(crate) revision: u64,
    pub(crate) status: RecordingStatus,
    pub(crate) elapsed_ms: u64,
    pub(crate) level_milli: u16,
    pub(crate) level_history: [u16; RECORDING_WAVEFORM_SAMPLES],
    pub(crate) output: Option<Arc<RecordingOutput>>,
    pub(crate) error: Option<String>,
}

impl Default for RecordingSnapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            status: RecordingStatus::Idle,
            elapsed_ms: 0,
            level_milli: 0,
            level_history: [0; RECORDING_WAVEFORM_SAMPLES],
            output: None,
            error: None,
        }
    }
}

type SharedSnapshot = Arc<Mutex<RecordingSnapshot>>;

pub(crate) struct AudioRecording {
    commands: mpsc::Sender<Command>,
    snapshot: SharedSnapshot,
    epoch: Arc<AtomicU64>,
    worker: Option<thread::JoinHandle<()>>,
}

impl AudioRecording {
    pub(crate) fn new() -> Self {
        Self::with_backend(Box::<CpalBackend>::default())
    }

    fn with_backend(backend: Box<dyn RecordingBackend>) -> Self {
        let (commands, receiver) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(RecordingSnapshot::default()));
        let epoch = Arc::new(AtomicU64::new(0));
        let worker = worker::spawn(receiver, snapshot.clone(), epoch.clone(), backend);
        Self {
            commands,
            snapshot,
            epoch,
            worker: Some(worker),
        }
    }

    pub(crate) fn snapshot(&self) -> RecordingSnapshot {
        self.snapshot
            .lock()
            .expect("audio recording state poisoned")
            .clone()
    }

    pub(crate) fn start(&self) {
        if self.snapshot().status.is_active() {
            return;
        }
        let epoch = self.epoch.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
        update_snapshot(&self.snapshot, |snapshot| {
            snapshot.status = RecordingStatus::RequestingPermission;
            snapshot.elapsed_ms = 0;
            snapshot.level_milli = 0;
            snapshot.level_history.fill(0);
            snapshot.output = None;
            snapshot.error = None;
        });
        self.send(Command::Start(epoch));
    }

    pub(crate) fn stop(&self) {
        if self.snapshot().status == RecordingStatus::Recording {
            update_snapshot(&self.snapshot, |snapshot| {
                snapshot.status = RecordingStatus::Finalizing;
                snapshot.level_milli = 0;
            });
            self.send(Command::Stop(self.epoch.load(Ordering::SeqCst)));
        }
    }

    pub(crate) fn cancel(&self) {
        if self.snapshot().status != RecordingStatus::Idle {
            self.epoch.fetch_add(1, Ordering::SeqCst);
            update_snapshot(&self.snapshot, reset_snapshot);
            self.send(Command::Cancel);
        }
    }

    pub(crate) fn reset(&self) {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        update_snapshot(&self.snapshot, reset_snapshot);
        self.send(Command::Reset);
    }

    fn send(&self, command: Command) {
        if self.commands.send(command).is_err() {
            update_snapshot(&self.snapshot, |snapshot| {
                snapshot.status = RecordingStatus::Failed;
                snapshot.error = Some("Audio recording stopped unexpectedly.".into());
            });
        }
    }
}

impl Drop for AudioRecording {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn reset_snapshot(snapshot: &mut RecordingSnapshot) {
    snapshot.status = RecordingStatus::Idle;
    snapshot.elapsed_ms = 0;
    snapshot.level_milli = 0;
    snapshot.level_history.fill(0);
    snapshot.output = None;
    snapshot.error = None;
}

fn update_snapshot(snapshot: &SharedSnapshot, update: impl FnOnce(&mut RecordingSnapshot)) {
    let mut snapshot = snapshot.lock().expect("audio recording state poisoned");
    let before = snapshot.clone();
    update(&mut snapshot);
    if *snapshot != before {
        snapshot.revision = snapshot.revision.wrapping_add(1);
    }
}
