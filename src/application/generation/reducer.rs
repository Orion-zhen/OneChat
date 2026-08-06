use std::time::Duration;

use crate::domain::{
    GenerationError, GenerationErrorKind, GenerationEvent, Message, MessageStatus, RequestError,
    RequestInfo, RequestStatus, now_timestamp,
};

pub fn apply_event(
    event: GenerationEvent,
    assistant: &mut Message,
    request: &mut RequestInfo,
    elapsed: Duration,
) -> bool {
    match event {
        GenerationEvent::Started => {
            request.status = RequestStatus::Streaming;
            false
        }
        GenerationEvent::TextDelta(delta) => {
            mark_first_token(request, elapsed);
            assistant.content.push_str(&delta);
            assistant.updated_at = now_timestamp();
            false
        }
        GenerationEvent::ThinkingDelta(delta) => {
            mark_first_token(request, elapsed);
            assistant.thinking.push_str(&delta);
            assistant.updated_at = now_timestamp();
            false
        }
        GenerationEvent::UsageUpdated(usage) => {
            request.usage = usage;
            false
        }
        GenerationEvent::Completed => {
            estimate_output_usage(assistant, request);
            assistant.status = MessageStatus::Completed;
            finish_request(request, RequestStatus::Completed, elapsed);
            true
        }
        GenerationEvent::Failed(error) => {
            estimate_output_usage(assistant, request);
            let cancelled = error.kind == GenerationErrorKind::UserCancelled;
            assistant.status = if cancelled {
                MessageStatus::Stopped
            } else {
                MessageStatus::Failed
            };
            request.error = (!cancelled).then(|| request_error(&error));
            finish_request(
                request,
                if cancelled {
                    RequestStatus::Stopped
                } else {
                    RequestStatus::Failed
                },
                elapsed,
            );
            true
        }
    }
}

pub fn interrupted_event() -> GenerationEvent {
    GenerationEvent::Failed(GenerationError::new(
        GenerationErrorKind::StreamInterrupted,
        "Provider stream closed unexpectedly",
    ))
}

fn mark_first_token(request: &mut RequestInfo, elapsed: Duration) {
    if request.ttft_ms.is_none() {
        request.first_token_at = Some(now_timestamp());
        request.ttft_ms = Some(elapsed.as_millis() as u64);
    }
    request.status = RequestStatus::Streaming;
}

fn finish_request(request: &mut RequestInfo, status: RequestStatus, elapsed: Duration) {
    request.status = status;
    request.finished_at = Some(now_timestamp());
    request.duration_ms = Some(elapsed.as_millis() as u64);
}

fn estimate_output_usage(assistant: &Message, request: &mut RequestInfo) {
    if request.usage.output_tokens.is_none() {
        request.usage.output_tokens = Some(estimate_tokens(
            assistant.content.chars().count() + assistant.thinking.chars().count(),
        ));
        request.usage.estimated = true;
    }
}

pub(super) fn estimate_tokens(characters: usize) -> u64 {
    characters.div_ceil(4) as u64
}

fn request_error(error: &GenerationError) -> RequestError {
    RequestError {
        kind: error.kind.as_str().into(),
        message: error.message.clone(),
        detail: error.detail.clone(),
    }
}
