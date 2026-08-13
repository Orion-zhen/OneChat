use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

use super::{
    PlaybackSnapshot, PlaybackSource, PlaybackStatus, backend::PlaybackBackend, update_snapshot,
};

pub(super) enum Command {
    Play {
        source_id: String,
        source: PlaybackSource,
        duration_ms: u64,
    },
    Pause,
    Resume,
    Seek(u64),
    Stop,
    Shutdown,
}

struct ActivePlayback {
    source_id: String,
    duration_ms: u64,
    elapsed: Duration,
    resumed_at: Option<Instant>,
}

impl ActivePlayback {
    fn position_ms(&self) -> u64 {
        let elapsed = self.elapsed
            + self
                .resumed_at
                .map(|started| started.elapsed())
                .unwrap_or_default();
        u64::try_from(elapsed.as_millis())
            .unwrap_or(u64::MAX)
            .min(self.duration_ms)
    }

    fn pause(&mut self) {
        if let Some(started) = self.resumed_at.take() {
            self.elapsed += started.elapsed();
        }
    }

    fn resume(&mut self) {
        if self.resumed_at.is_none() {
            self.resumed_at = Some(Instant::now());
        }
    }

    fn seek(&mut self, position_ms: u64) {
        self.elapsed = Duration::from_millis(position_ms.min(self.duration_ms));
        if self.resumed_at.is_some() {
            self.resumed_at = Some(Instant::now());
        }
    }
}

pub(super) fn run(
    commands: mpsc::Receiver<Command>,
    snapshot: Arc<Mutex<PlaybackSnapshot>>,
    mut backend: Box<dyn PlaybackBackend>,
) {
    let mut active: Option<ActivePlayback> = None;
    loop {
        match commands.recv_timeout(Duration::from_millis(40)) {
            Ok(Command::Play {
                source_id,
                source,
                duration_ms,
            }) => {
                backend.stop();
                active = None;
                update_snapshot(&snapshot, |snapshot| {
                    snapshot.source_id = Some(source_id.clone());
                    snapshot.status = PlaybackStatus::Loading;
                    snapshot.position_ms = 0;
                    snapshot.duration_ms = duration_ms;
                    snapshot.error = None;
                });
                match backend.start(source) {
                    Ok(()) => {
                        active = Some(ActivePlayback {
                            source_id,
                            duration_ms,
                            elapsed: Duration::ZERO,
                            resumed_at: Some(Instant::now()),
                        });
                        update_snapshot(&snapshot, |snapshot| {
                            snapshot.status = PlaybackStatus::Playing;
                        });
                    }
                    Err(error) => update_snapshot(&snapshot, |snapshot| {
                        snapshot.status = PlaybackStatus::Failed;
                        snapshot.error = Some(error);
                    }),
                }
            }
            Ok(Command::Pause) => {
                if let Some(active) = active.as_mut()
                    && active.resumed_at.is_some()
                {
                    active.pause();
                    backend.pause();
                    let position_ms = active.position_ms();
                    update_snapshot(&snapshot, |snapshot| {
                        snapshot.status = PlaybackStatus::Paused;
                        snapshot.position_ms = position_ms;
                    });
                }
            }
            Ok(Command::Resume) => {
                if let Some(active) = active.as_mut()
                    && active.resumed_at.is_none()
                {
                    active.resume();
                    backend.resume();
                    update_snapshot(&snapshot, |snapshot| {
                        snapshot.status = PlaybackStatus::Playing;
                    });
                }
            }
            Ok(Command::Seek(position_ms)) => {
                if let Some(active) = active.as_mut() {
                    let position_ms = position_ms.min(active.duration_ms);
                    match backend.seek(Duration::from_millis(position_ms)) {
                        Ok(()) => {
                            active.seek(position_ms);
                            update_snapshot(&snapshot, |snapshot| {
                                snapshot.position_ms = position_ms;
                                snapshot.error = None;
                            });
                        }
                        Err(error) => update_snapshot(&snapshot, |snapshot| {
                            snapshot.error = Some(error);
                        }),
                    }
                }
            }
            Ok(Command::Stop) => stop_active(&mut active, backend.as_mut(), &snapshot),
            Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                backend.stop();
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        if let Some(current) = active.as_ref() {
            if backend.finished() {
                stop_active(&mut active, backend.as_mut(), &snapshot);
            } else if current.resumed_at.is_some() {
                let source_id = current.source_id.clone();
                let position_ms = current.position_ms();
                update_snapshot(&snapshot, |snapshot| {
                    if snapshot.source_id.as_deref() == Some(&source_id) {
                        snapshot.position_ms = position_ms;
                    }
                });
            }
        }
    }
}

fn stop_active(
    active: &mut Option<ActivePlayback>,
    backend: &mut dyn PlaybackBackend,
    snapshot: &Arc<Mutex<PlaybackSnapshot>>,
) {
    backend.stop();
    *active = None;
    update_snapshot(snapshot, |snapshot| {
        snapshot.source_id = None;
        snapshot.status = PlaybackStatus::Idle;
        snapshot.position_ms = 0;
        snapshot.duration_ms = 0;
        snapshot.error = None;
    });
}
