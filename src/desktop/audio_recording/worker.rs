use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use crate::application::attachments::MAX_AUDIO_BYTES;

use super::{
    RecordingLimit, RecordingOutput, RecordingStatus, SharedSnapshot,
    capture::{InputSession, RecordingBackend},
    pipeline::RecordingBuffer,
    reset_snapshot, update_snapshot,
};

pub(super) enum Command {
    Start(u64),
    Stop(u64),
    Cancel,
    Reset,
    Shutdown,
}

struct ActiveRecording {
    epoch: u64,
    input: InputSession,
    buffer: RecordingBuffer,
    level_milli: u16,
}

pub(super) fn spawn(
    commands: mpsc::Receiver<Command>,
    snapshot: SharedSnapshot,
    epoch: Arc<AtomicU64>,
    backend: Box<dyn RecordingBackend>,
) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("onechat-audio-recording".into())
        .spawn(move || run(commands, snapshot, epoch, backend))
        .expect("failed to start audio recording worker")
}

fn run(
    commands: mpsc::Receiver<Command>,
    snapshot: SharedSnapshot,
    epoch: Arc<AtomicU64>,
    mut backend: Box<dyn RecordingBackend>,
) {
    let mut active: Option<ActiveRecording> = None;
    loop {
        match commands.recv_timeout(Duration::from_millis(20)) {
            Ok(Command::Start(command_epoch)) => {
                if active.is_some() {
                    continue;
                }
                match catch_unwind(AssertUnwindSafe(|| backend.start())) {
                    Ok(Ok(input)) if epoch.load(Ordering::SeqCst) == command_epoch => {
                        let sample_rate = input.sample_rate;
                        active = Some(ActiveRecording {
                            epoch: command_epoch,
                            input,
                            buffer: RecordingBuffer::new(sample_rate),
                            level_milli: 0,
                        });
                        update_snapshot(&snapshot, |snapshot| {
                            snapshot.status = RecordingStatus::Recording;
                            snapshot.error = None;
                        });
                    }
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) if epoch.load(Ordering::SeqCst) == command_epoch => {
                        fail(&mut active, &snapshot, error)
                    }
                    Err(_) if epoch.load(Ordering::SeqCst) == command_epoch => fail(
                        &mut active,
                        &snapshot,
                        "Could not initialize microphone capture.".into(),
                    ),
                    Ok(Err(_)) | Err(_) => {}
                }
            }
            Ok(Command::Stop(command_epoch)) => {
                finalize(&mut active, &snapshot, &epoch, command_epoch, None)
            }
            Ok(Command::Cancel) | Ok(Command::Reset) => {
                active = None;
                update_snapshot(&snapshot, reset_snapshot);
            }
            Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        if active
            .as_ref()
            .is_some_and(|recording| recording.epoch != epoch.load(Ordering::SeqCst))
        {
            active = None;
        }
        let Some(recording) = active.as_mut() else {
            continue;
        };
        if let Ok(error) = recording.input.errors.try_recv() {
            fail(&mut active, &snapshot, error);
            continue;
        }

        let mut limit = None;
        while let Ok(samples) = recording.input.samples.try_recv() {
            let peak = samples
                .iter()
                .copied()
                .map(f32::abs)
                .fold(0.0, f32::max)
                .clamp(0.0, 1.0);
            let target = (peak * 1_000.0).round() as u16;
            recording.level_milli = recording.level_milli.saturating_mul(3) / 4 + target / 4;
            if let Some(reached) = recording.buffer.push(&samples) {
                limit = Some(reached);
                break;
            }
        }
        if let Some(limit) = limit {
            let recording_epoch = recording.epoch;
            finalize(&mut active, &snapshot, &epoch, recording_epoch, Some(limit));
        } else {
            let elapsed_ms = recording.buffer.elapsed_ms();
            let level_milli = recording.level_milli;
            update_snapshot(&snapshot, |snapshot| {
                snapshot.elapsed_ms = elapsed_ms;
                snapshot.level_milli = level_milli;
                snapshot.level_history.rotate_left(1);
                *snapshot.level_history.last_mut().unwrap() = level_milli;
            });
        }
    }
}

fn finalize(
    active: &mut Option<ActiveRecording>,
    snapshot: &SharedSnapshot,
    epoch: &Arc<AtomicU64>,
    expected_epoch: u64,
    limit: Option<RecordingLimit>,
) {
    let Some(recording) = active.take() else {
        return;
    };
    if recording.epoch != expected_epoch || epoch.load(Ordering::SeqCst) != expected_epoch {
        return;
    }
    update_snapshot(snapshot, |snapshot| {
        snapshot.status = RecordingStatus::Finalizing;
        snapshot.level_milli = 0;
    });
    if recording.buffer.is_empty() {
        fail(active, snapshot, "The microphone captured no audio.".into());
        return;
    }
    let duration_ms = recording.buffer.elapsed_ms();
    match recording.buffer.encode_wav() {
        _ if epoch.load(Ordering::SeqCst) != expected_epoch => {}
        Ok(wav) if wav.len() as u64 <= MAX_AUDIO_BYTES => {
            let output = Arc::new(RecordingOutput {
                wav,
                duration_ms,
                limit,
            });
            update_snapshot(snapshot, |snapshot| {
                snapshot.status = RecordingStatus::Completed;
                snapshot.elapsed_ms = duration_ms;
                snapshot.output = Some(output);
                snapshot.error = None;
            });
        }
        Ok(_) => fail(
            active,
            snapshot,
            "The recording exceeded the 10 MiB audio limit.".into(),
        ),
        Err(error) => fail(active, snapshot, error),
    }
}

fn fail(active: &mut Option<ActiveRecording>, snapshot: &SharedSnapshot, error: String) {
    *active = None;
    update_snapshot(snapshot, |snapshot| {
        snapshot.status = RecordingStatus::Failed;
        snapshot.level_milli = 0;
        snapshot.output = None;
        snapshot.error = Some(error);
    });
}

#[cfg(test)]
mod tests;
