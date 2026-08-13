use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use super::{PlaybackSnapshot, PlaybackSource, PlaybackStatus, backend::PlaybackBackend, *};

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

    fn seek(&mut self, _: Duration) -> Result<(), String> {
        self.events.lock().unwrap().push("seek");
        Ok(())
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
        state.source_id.as_deref() == Some("second") && state.status == PlaybackStatus::Playing
    });

    assert_eq!(
        events.lock().unwrap().as_slice(),
        ["stop", "start", "pause", "resume", "stop", "start"]
    );
}

#[test]
fn seek_updates_the_active_position_without_changing_its_source() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let player = AudioPlayback::with_backend(Box::new(FakeBackend {
        events: events.clone(),
        finished: Arc::new(AtomicBool::new(false)),
        fail_start: false,
    }));
    player.play("audio".into(), PlaybackSource::Bytes(vec![1]), 5_000);
    wait_for(&player, |state| state.status == PlaybackStatus::Playing);
    player.seek(3_250);
    wait_for(&player, |state| state.position_ms >= 3_250);

    let state = player.snapshot();
    assert_eq!(state.source_id.as_deref(), Some("audio"));
    assert!(state.position_ms >= 3_250);
    assert!(events.lock().unwrap().contains(&"seek"));
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
    assert_eq!(player.snapshot().source_id, None);
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
    assert_eq!(state.source_id.as_deref(), Some("audio"));
    assert_eq!(
        state.error.as_deref(),
        Some("No audio output device is available.")
    );
}
