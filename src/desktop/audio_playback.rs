use std::{
    fs::File,
    io::{BufReader, Cursor},
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

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
    pub(crate) attachment_id: Option<String>,
    pub(crate) status: PlaybackStatus,
    pub(crate) position_ms: u64,
    pub(crate) duration_ms: u64,
    pub(crate) error: Option<String>,
}

pub(crate) enum PlaybackSource {
    Bytes(Vec<u8>),
    File(PathBuf),
}

enum Command {
    Play {
        attachment_id: String,
        source: PlaybackSource,
        duration_ms: u64,
    },
    Pause,
    Resume,
    Stop,
    Shutdown,
}

trait PlaybackBackend: Send {
    fn start(&mut self, source: PlaybackSource) -> Result<(), String>;
    fn pause(&mut self);
    fn resume(&mut self);
    fn stop(&mut self);
    fn finished(&self) -> bool;
}

#[derive(Default)]
struct RodioBackend {
    output: Option<MixerDeviceSink>,
    player: Option<Player>,
}

impl PlaybackBackend for RodioBackend {
    fn start(&mut self, source: PlaybackSource) -> Result<(), String> {
        self.stop();
        let mut output = DeviceSinkBuilder::open_default_sink()
            .map_err(|error| format!("Could not open the audio output device: {error}"))?;
        output.log_on_drop(false);
        let player = Player::connect_new(output.mixer());
        match source {
            PlaybackSource::Bytes(bytes) => {
                let decoder = Decoder::try_from(Cursor::new(bytes))
                    .map_err(|error| format!("Could not decode audio: {error}"))?;
                player.append(decoder);
            }
            PlaybackSource::File(path) => {
                let file = File::open(&path).map_err(|error| {
                    format!("Could not open audio file {}: {error}", path.display())
                })?;
                let decoder = Decoder::try_from(BufReader::new(file))
                    .map_err(|error| format!("Could not decode audio: {error}"))?;
                player.append(decoder);
            }
        }
        player.play();
        self.output = Some(output);
        self.player = Some(player);
        Ok(())
    }

    fn pause(&mut self) {
        if let Some(player) = &self.player {
            player.pause();
        }
    }

    fn resume(&mut self) {
        if let Some(player) = &self.player {
            player.play();
        }
    }

    fn stop(&mut self) {
        if let Some(player) = self.player.take() {
            player.stop();
        }
        self.output = None;
    }

    fn finished(&self) -> bool {
        self.player.as_ref().is_some_and(Player::empty)
    }
}

struct ActivePlayback {
    attachment_id: String,
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
            .spawn(move || run_worker(receiver, worker_snapshot, backend))
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

    pub(crate) fn play(&self, attachment_id: String, source: PlaybackSource, duration_ms: u64) {
        self.send(Command::Play {
            attachment_id,
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

fn run_worker(
    commands: mpsc::Receiver<Command>,
    snapshot: Arc<Mutex<PlaybackSnapshot>>,
    mut backend: Box<dyn PlaybackBackend>,
) {
    let mut active: Option<ActivePlayback> = None;
    loop {
        match commands.recv_timeout(Duration::from_millis(40)) {
            Ok(Command::Play {
                attachment_id,
                source,
                duration_ms,
            }) => {
                backend.stop();
                active = None;
                update_snapshot(&snapshot, |snapshot| {
                    snapshot.attachment_id = Some(attachment_id.clone());
                    snapshot.status = PlaybackStatus::Loading;
                    snapshot.position_ms = 0;
                    snapshot.duration_ms = duration_ms;
                    snapshot.error = None;
                });
                match backend.start(source) {
                    Ok(()) => {
                        active = Some(ActivePlayback {
                            attachment_id,
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
                let attachment_id = current.attachment_id.clone();
                let position_ms = current.position_ms();
                update_snapshot(&snapshot, |snapshot| {
                    if snapshot.attachment_id.as_deref() == Some(&attachment_id) {
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
        snapshot.attachment_id = None;
        snapshot.status = PlaybackStatus::Idle;
        snapshot.position_ms = 0;
        snapshot.duration_ms = 0;
        snapshot.error = None;
    });
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    struct FakeBackend {
        events: Arc<Mutex<Vec<&'static str>>>,
        finished: Arc<AtomicBool>,
        fail_start: bool,
    }

    impl PlaybackBackend for FakeBackend {
        fn start(&mut self, _: PlaybackSource) -> Result<(), String> {
            self.events.lock().unwrap().push("start");
            if self.fail_start {
                Err("No audio output device is available.".into())
            } else {
                Ok(())
            }
        }

        fn pause(&mut self) {
            self.events.lock().unwrap().push("pause");
        }

        fn resume(&mut self) {
            self.events.lock().unwrap().push("resume");
        }

        fn stop(&mut self) {
            self.events.lock().unwrap().push("stop");
        }

        fn finished(&self) -> bool {
            self.finished.load(Ordering::Relaxed)
        }
    }

    fn wait_for(player: &AudioPlayback, predicate: impl Fn(&PlaybackSnapshot) -> bool) {
        for _ in 0..100 {
            if predicate(&player.snapshot()) {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("playback state did not update: {:?}", player.snapshot());
    }

    #[test]
    fn switching_resources_stops_the_old_resource() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let player = AudioPlayback::with_backend(Box::new(FakeBackend {
            events: events.clone(),
            finished: Arc::new(AtomicBool::new(false)),
            fail_start: false,
        }));

        player.play("first".into(), PlaybackSource::Bytes(vec![1]), 2_000);
        wait_for(&player, |state| state.status == PlaybackStatus::Playing);
        player.pause();
        wait_for(&player, |state| state.status == PlaybackStatus::Paused);
        player.resume();
        wait_for(&player, |state| state.status == PlaybackStatus::Playing);
        player.play("second".into(), PlaybackSource::Bytes(vec![2]), 3_000);
        wait_for(&player, |state| {
            state.attachment_id.as_deref() == Some("second")
                && state.status == PlaybackStatus::Playing
        });

        assert_eq!(
            events.lock().unwrap().as_slice(),
            ["stop", "start", "pause", "resume", "stop", "start"]
        );
    }

    #[test]
    fn completion_and_explicit_stop_release_the_resource() {
        let finished = Arc::new(AtomicBool::new(false));
        let events = Arc::new(Mutex::new(Vec::new()));
        let player = AudioPlayback::with_backend(Box::new(FakeBackend {
            events: events.clone(),
            finished: finished.clone(),
            fail_start: false,
        }));
        player.play("audio".into(), PlaybackSource::Bytes(vec![1]), 2_000);
        wait_for(&player, |state| state.status == PlaybackStatus::Playing);
        finished.store(true, Ordering::Relaxed);
        wait_for(&player, |state| state.status == PlaybackStatus::Idle);

        finished.store(false, Ordering::Relaxed);
        player.play("audio-again".into(), PlaybackSource::Bytes(vec![2]), 2_000);
        wait_for(&player, |state| state.status == PlaybackStatus::Playing);
        player.stop();
        wait_for(&player, |state| state.status == PlaybackStatus::Idle);

        assert!(events.lock().unwrap().ends_with(&["stop"]));
        assert_eq!(player.snapshot().attachment_id, None);
    }

    #[test]
    fn backend_failures_are_reported_without_an_active_resource() {
        let player = AudioPlayback::with_backend(Box::new(FakeBackend {
            events: Arc::new(Mutex::new(Vec::new())),
            finished: Arc::new(AtomicBool::new(false)),
            fail_start: true,
        }));
        player.play("audio".into(), PlaybackSource::Bytes(vec![1]), 2_000);
        wait_for(&player, |state| state.status == PlaybackStatus::Failed);

        let state = player.snapshot();
        assert_eq!(state.attachment_id.as_deref(), Some("audio"));
        assert_eq!(
            state.error.as_deref(),
            Some("No audio output device is available.")
        );
    }
}
