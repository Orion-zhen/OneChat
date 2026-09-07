use std::time::Duration;

use async_channel::{Receiver, TryRecvError};

use crate::domain::{AssistantResponse, GenerationEvent, Message, RequestInfo};

use super::{apply_event, continuation::ContinuationNormalizer, interrupted_event};

pub const UI_FLUSH_INTERVAL: Duration = Duration::from_millis(40);

#[derive(Clone)]
pub struct GenerationSnapshot {
    pub response: AssistantResponse,
    pub request: RequestInfo,
    pub terminal: bool,
    pub finished_reasoning_ids: Vec<String>,
}

pub struct GenerationStream {
    pub snapshot: GenerationSnapshot,
    receiver: Receiver<GenerationEvent>,
    continuation: Option<ContinuationNormalizer>,
}

impl GenerationStream {
    pub fn new(
        receiver: Receiver<GenerationEvent>,
        response: AssistantResponse,
        request: RequestInfo,
    ) -> Self {
        Self {
            snapshot: GenerationSnapshot {
                response,
                request,
                terminal: false,
                finished_reasoning_ids: Vec::new(),
            },
            receiver,
            continuation: None,
        }
    }

    pub(super) fn with_continuation(mut self, prefill: Option<&Message>) -> Self {
        self.continuation = Some(ContinuationNormalizer::new(prefill));
        self
    }

    pub fn drain(&mut self, elapsed: Duration) -> bool {
        if self.snapshot.terminal {
            return false;
        }
        self.snapshot.finished_reasoning_ids.clear();
        let mut changed = false;
        // Bound each batch so an active producer cannot delay UI and storage flushes.
        // The extra receive detects a channel closed just after its queued events.
        let pending = self.receiver.len();
        for _ in 0..=pending {
            let event = match self.receiver.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Closed) => interrupted_event(),
            };
            changed = true;
            if let Some(normalizer) = &mut self.continuation {
                for event in normalizer.normalize(event) {
                    self.apply(event, elapsed);
                }
            } else {
                self.apply(event, elapsed);
            }
            if self.snapshot.terminal {
                break;
            }
        }
        changed
    }

    fn apply(&mut self, event: GenerationEvent, elapsed: Duration) {
        let outcome = apply_event(
            event,
            &mut self.snapshot.response,
            &mut self.snapshot.request,
            elapsed,
        );
        self.snapshot.terminal |= outcome.terminal;
        self.snapshot
            .finished_reasoning_ids
            .extend(outcome.finished_reasoning_id);
    }
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
