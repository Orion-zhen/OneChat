mod backend;
mod worker;

#[cfg(test)]
mod tests;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
};

use crate::speech::AudioClip;

use self::{
    backend::{PlaybackBackend, RodioBackend},
    worker::Command,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum PlaybackStatus {
    #[default]
    Idle,
    Loading,
    Playing,
    Paused,
    Failed,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlaybackSnapshot {
    pub(crate) revision: u64,
    pub(crate) source_id: Option<String>,
    pub(crate) status: PlaybackStatus,
    pub(crate) position_ms: u64,
    pub(crate) duration_ms: u64,
    pub(crate) error: Option<String>,
}

pub(crate) enum PlaybackSource {
    Bytes(Vec<u8>),
    File(PathBuf),
    Clip(AudioClip),
}

pub(crate) struct AudioPlayback {
    commands: mpsc::Sender<Command>,
    snapshot: Arc<Mutex<PlaybackSnapshot>>,
    worker: Option<thread::JoinHandle<()>>,
}

impl AudioPlayback {
    pub(crate) fn new() -> Self {
        Self::with_backend(Box::<RodioBackend>::default())
    }

    fn with_backend(backend: Box<dyn PlaybackBackend>) -> Self {
        let (commands, receiver) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(PlaybackSnapshot::default()));
        let worker_snapshot = snapshot.clone();
        let worker = thread::Builder::new()
            .name("onechat-audio-playback".into())
            .spawn(move || worker::run(receiver, worker_snapshot, backend))
            .expect("failed to start audio playback worker");
        Self {
            commands,
            snapshot,
            worker: Some(worker),
        }
    }

    pub(crate) fn snapshot(&self) -> PlaybackSnapshot {
        self.snapshot
            .lock()
            .expect("audio playback state poisoned")
            .clone()
    }

    pub(crate) fn play(&self, source_id: String, source: PlaybackSource, duration_ms: u64) {
        self.send(Command::Play {
            source_id,
            source,
            duration_ms,
        });
    }

    pub(crate) fn pause(&self) {
        self.send(Command::Pause);
    }

    pub(crate) fn resume(&self) {
        self.send(Command::Resume);
    }

    pub(crate) fn seek(&self, position_ms: u64) {
        self.send(Command::Seek(position_ms));
    }

    pub(crate) fn stop(&self) {
        self.send(Command::Stop);
    }

    fn send(&self, command: Command) {
        if self.commands.send(command).is_err() {
            update_snapshot(&self.snapshot, |snapshot| {
                snapshot.status = PlaybackStatus::Failed;
                snapshot.error = Some("Audio playback stopped unexpectedly.".into());
            });
        }
    }
}

impl Drop for AudioPlayback {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn update_snapshot(
    snapshot: &Arc<Mutex<PlaybackSnapshot>>,
    update: impl FnOnce(&mut PlaybackSnapshot),
) {
    let mut snapshot = snapshot.lock().expect("audio playback state poisoned");
    let before = snapshot.clone();
    update(&mut snapshot);
    if *snapshot != before {
        snapshot.revision = snapshot.revision.wrapping_add(1);
    }
}
