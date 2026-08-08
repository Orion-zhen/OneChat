use std::time::Duration;

use crate::domain::{
    AssistantResponse, GenerationError, GenerationErrorKind, GenerationEvent, MessageStatus,
    RequestError, RequestInfo, RequestStatus, now_timestamp,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EventOutcome {
    pub terminal: bool,
    pub thinking_finished: bool,
}

pub fn apply_event(
    event: GenerationEvent,
    assistant: &mut AssistantResponse,
    request: &mut RequestInfo,
    elapsed: Duration,
) -> EventOutcome {
    match event {
        GenerationEvent::Started => {
            request.status = RequestStatus::Streaming;
            EventOutcome::default()
        }
        GenerationEvent::TextDelta(delta) => {
            mark_first_token(request, elapsed);
            let thinking_finished = !delta.is_empty()
                && assistant.content.is_empty()
                && !assistant.thinking.is_empty()
                && finish_thinking(assistant, request, elapsed);
            assistant.content.push_str(&delta);
            assistant.updated_at = now_timestamp();
            EventOutcome {
                thinking_finished,
                ..EventOutcome::default()
            }
        }
        GenerationEvent::ThinkingDelta(delta) => {
            mark_first_token(request, elapsed);
            assistant.thinking.push_str(&delta);
            assistant.updated_at = now_timestamp();
            EventOutcome::default()
        }
        GenerationEvent::UsageUpdated(usage) => {
            if request.usage.estimated || usage.estimated {
                request.usage = usage;
            } else {
                request.usage.input_tokens =
                    sum_tokens(request.usage.input_tokens, usage.input_tokens);
                request.usage.output_tokens =
                    sum_tokens(request.usage.output_tokens, usage.output_tokens);
            }
            EventOutcome::default()
        }
        GenerationEvent::ProviderOutput => {
            mark_first_token(request, elapsed);
            EventOutcome::default()
        }
        GenerationEvent::ToolExecutionUpdated(execution) => {
            if let Some(stored) = assistant
                .tool_executions
                .iter_mut()
                .find(|stored| stored.id == execution.id)
            {
                *stored = *execution;
            } else {
                assistant.tool_executions.push(*execution);
            }
            request.tool_call_count = assistant.tool_executions.len() as u64;
            request.tool_duration_ms = assistant
                .tool_executions
                .iter()
                .filter_map(|execution| execution.duration_ms)
                .reduce(u64::saturating_add);
            request.status = RequestStatus::Streaming;
            assistant.updated_at = now_timestamp();
            EventOutcome::default()
        }
        GenerationEvent::TranscriptAppended(message) => {
            if matches!(message.as_ref(), crate::domain::Message::Assistant { .. }) {
                mark_first_token(request, elapsed);
            }
            assistant.transcript.push(*message);
            assistant.updated_at = now_timestamp();
            EventOutcome::default()
        }
        GenerationEvent::Completed => {
            let thinking_finished = finish_thinking(assistant, request, elapsed);
            estimate_output_usage(assistant, request);
            assistant.status = MessageStatus::Completed;
            finish_request(request, RequestStatus::Completed, elapsed);
            EventOutcome {
                terminal: true,
                thinking_finished,
            }
        }
        GenerationEvent::Failed(error) => {
            let thinking_finished = finish_thinking(assistant, request, elapsed);
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
            EventOutcome {
                terminal: true,
                thinking_finished,
            }
        }
    }
}

pub fn interrupted_event() -> GenerationEvent {
    GenerationEvent::Failed(GenerationError::new(
        GenerationErrorKind::StreamInterrupted,
        "Provider stream closed unexpectedly",
    ))
}

fn sum_tokens(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.saturating_add(right)),
        (left, right) => left.or(right),
    }
}

fn mark_first_token(request: &mut RequestInfo, elapsed: Duration) {
    if request.ttft_ms.is_none() {
        request.first_token_at = Some(now_timestamp());
        request.ttft_ms = Some(elapsed.as_millis() as u64);
    }
    request.status = RequestStatus::Streaming;
}

fn finish_thinking(
    assistant: &AssistantResponse,
    request: &mut RequestInfo,
    elapsed: Duration,
) -> bool {
    if assistant.thinking.is_empty() || request.thinking_duration_ms.is_some() {
        return false;
    }
    request.thinking_duration_ms = Some(elapsed.as_millis() as u64);
    true
}

fn finish_request(request: &mut RequestInfo, status: RequestStatus, elapsed: Duration) {
    request.status = status;
    request.finished_at = Some(now_timestamp());
    request.duration_ms = Some(elapsed.as_millis() as u64);
}

fn estimate_output_usage(assistant: &AssistantResponse, request: &mut RequestInfo) {
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
