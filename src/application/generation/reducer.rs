use std::time::Duration;

use crate::domain::{
    AssistantResponse, GenerationError, GenerationErrorKind, GenerationEvent, MessageStatus,
    RequestError, RequestInfo, RequestStatus, continue_last_assistant, now_timestamp,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EventOutcome {
    pub terminal: bool,
    pub finished_reasoning_id: Option<String>,
}

pub fn apply_event(
    event: GenerationEvent,
    assistant: &mut AssistantResponse,
    request: &mut RequestInfo,
    elapsed: Duration,
) -> EventOutcome {
    let elapsed_ms = elapsed.as_millis() as u64;
    match event {
        GenerationEvent::Started => {
            request.status = RequestStatus::Streaming;
            EventOutcome::default()
        }
        GenerationEvent::TextDelta(delta) => {
            mark_first_token(request, elapsed);
            let finished_reasoning_id = (!delta.is_empty())
                .then(|| assistant.append_output(&delta, elapsed_ms))
                .flatten();
            record_thinking_duration(request, elapsed, finished_reasoning_id.is_some());
            assistant.updated_at = now_timestamp();
            EventOutcome {
                finished_reasoning_id,
                ..EventOutcome::default()
            }
        }
        GenerationEvent::ThinkingDelta { provider_id, delta } => {
            mark_first_token(request, elapsed);
            let finished_reasoning_id = (!delta.is_empty())
                .then(|| assistant.append_reasoning(provider_id, &delta, elapsed_ms))
                .flatten();
            record_thinking_duration(request, elapsed, finished_reasoning_id.is_some());
            assistant.updated_at = now_timestamp();
            EventOutcome {
                finished_reasoning_id,
                ..EventOutcome::default()
            }
        }
        GenerationEvent::ToolCallObserved {
            internal_call_id,
            provider_tool_call_id,
        } => {
            mark_first_token(request, elapsed);
            let finished_reasoning_id =
                assistant.observe_tool_call(internal_call_id, provider_tool_call_id, elapsed_ms);
            record_thinking_duration(request, elapsed, finished_reasoning_id.is_some());
            assistant.updated_at = now_timestamp();
            EventOutcome {
                finished_reasoning_id,
                ..EventOutcome::default()
            }
        }
        GenerationEvent::StepStarted {
            estimated_input_tokens,
        } => {
            let finished_reasoning_id = assistant.finish_reasoning(elapsed_ms);
            record_thinking_duration(request, elapsed, finished_reasoning_id.is_some());
            request.last_step_input_tokens = None;
            request.last_step_estimated_input_tokens = Some(estimated_input_tokens);
            EventOutcome {
                finished_reasoning_id,
                ..EventOutcome::default()
            }
        }
        GenerationEvent::UsageUpdated(usage) => {
            request.last_step_input_tokens = usage.input_tokens;
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
        GenerationEvent::ToolExecutionUpdated(execution) => {
            let finished_reasoning_id = assistant.upsert_tool_execution(*execution, elapsed_ms);
            record_thinking_duration(request, elapsed, finished_reasoning_id.is_some());
            request.tool_call_count = assistant.tool_executions.len() as u64;
            request.tool_duration_ms = assistant
                .tool_executions
                .iter()
                .filter_map(|execution| execution.duration_ms)
                .reduce(u64::saturating_add);
            request.status = RequestStatus::Streaming;
            assistant.updated_at = now_timestamp();
            EventOutcome {
                finished_reasoning_id,
                ..EventOutcome::default()
            }
        }
        GenerationEvent::TranscriptAppended(message) => {
            if matches!(message.as_ref(), crate::domain::Message::Assistant { .. }) {
                mark_first_token(request, elapsed);
            }
            assistant.transcript.push(*message);
            assistant.updated_at = now_timestamp();
            EventOutcome::default()
        }
        GenerationEvent::TranscriptContinued(message) => {
            mark_first_token(request, elapsed);
            continue_last_assistant(&mut assistant.transcript, *message);
            assistant.updated_at = now_timestamp();
            EventOutcome::default()
        }
        GenerationEvent::Completed => {
            let finished_reasoning_id = assistant.finish_reasoning(elapsed_ms);
            record_thinking_duration(request, elapsed, finished_reasoning_id.is_some());
            estimate_output_usage(assistant, request);
            assistant.status = MessageStatus::Completed;
            finish_request(request, RequestStatus::Completed, elapsed);
            EventOutcome {
                terminal: true,
                finished_reasoning_id,
            }
        }
        GenerationEvent::Failed(error) => {
            let finished_reasoning_id = assistant.finish_reasoning(elapsed_ms);
            record_thinking_duration(request, elapsed, finished_reasoning_id.is_some());
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
                finished_reasoning_id,
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

fn record_thinking_duration(
    request: &mut RequestInfo,
    elapsed: Duration,
    reasoning_finished: bool,
) {
    if reasoning_finished && request.thinking_duration_ms.is_none() {
        request.thinking_duration_ms = Some(elapsed.as_millis() as u64);
    }
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
