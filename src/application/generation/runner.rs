use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use async_channel::Sender;
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{Message, RequestInfo},
    providers,
    storage::{Storage, StorageError},
};

use super::{PreparedGeneration, apply_event, interrupted_event};

pub const UI_FLUSH_INTERVAL: Duration = Duration::from_millis(40);
pub const STORAGE_FLUSH_INTERVAL: Duration = Duration::from_millis(320);

pub struct GenerationSnapshot {
    pub assistant: Message,
    pub request: RequestInfo,
    pub terminal: bool,
}

pub enum GenerationUpdate {
    Snapshot(Box<GenerationSnapshot>),
    PersistenceFailed(StorageError),
}

pub async fn run_generation(
    prepared: PreparedGeneration,
    storage: Arc<Storage>,
    cancellation: CancellationToken,
    updates: Sender<GenerationUpdate>,
) {
    let (event_sender, event_receiver) = async_channel::bounded(256);
    tokio::spawn(providers::generate(
        prepared.provider_request,
        event_sender,
        cancellation,
    ));

    let mut assistant = prepared.assistant;
    let mut request = prepared.request_info;
    let started = Instant::now();
    let mut last_storage_flush = Instant::now();
    let mut terminal = false;

    loop {
        tokio::time::sleep(UI_FLUSH_INTERVAL).await;
        let mut events = Vec::new();
        while let Ok(event) = event_receiver.try_recv() {
            events.push(event);
        }
        if events.is_empty() && event_receiver.is_closed() && !terminal {
            events.push(interrupted_event());
        }
        if events.is_empty() {
            continue;
        }

        for event in events {
            terminal |= apply_event(event, &mut assistant, &mut request, started.elapsed());
        }

        if terminal || last_storage_flush.elapsed() >= STORAGE_FLUSH_INTERVAL {
            let storage = storage.clone();
            let saved_assistant = assistant.clone();
            let saved_request = request.clone();
            let result = tokio::task::spawn_blocking(move || {
                storage.persist_generation(&saved_assistant, &saved_request)
            })
            .await;
            last_storage_flush = Instant::now();
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    if updates
                        .send(GenerationUpdate::PersistenceFailed(error))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                Err(error) => {
                    let storage_error = StorageError::InvalidData(format!(
                        "generation persistence task failed: {error}"
                    ));
                    if updates
                        .send(GenerationUpdate::PersistenceFailed(storage_error))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }
        }

        if updates
            .send(GenerationUpdate::Snapshot(Box::new(GenerationSnapshot {
                assistant: assistant.clone(),
                request: request.clone(),
                terminal,
            })))
            .await
            .is_err()
        {
            return;
        }
        if terminal {
            return;
        }
    }
}
