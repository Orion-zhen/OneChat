use async_channel::Sender;

use super::*;
use crate::domain::{
    GenerationError, GenerationErrorKind, MessageStatus, Model, Provider, ProviderKind,
    RequestStatus,
};

fn stream() -> (Sender<GenerationEvent>, GenerationStream) {
    let provider = Provider::new("Test", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test", "Test", provider.kind);
    let mut response = AssistantResponse::new(&model, &provider);
    response.status = MessageStatus::Streaming;
    let request = RequestInfo::new("conversation", "turn", &response.id);
    let (sender, receiver) = async_channel::bounded(256);
    (sender, GenerationStream::new(receiver, response, request))
}

#[test]
fn empty_open_stream_waits_and_reasoning_completion_is_reported_once() {
    let (sender, mut stream) = stream();
    assert!(!stream.drain(Duration::ZERO));
    assert!(!stream.snapshot.terminal);
    sender.try_send(GenerationEvent::Started).unwrap();
    sender
        .try_send(GenerationEvent::ThinkingDelta {
            provider_id: None,
            delta: "reason".into(),
        })
        .unwrap();
    assert!(stream.drain(Duration::from_millis(10)));
    assert_eq!(stream.snapshot.request.ttft_ms, Some(10));
    assert!(stream.snapshot.finished_reasoning_ids.is_empty());
    sender
        .try_send(GenerationEvent::TextDelta("answer".into()))
        .unwrap();
    assert!(stream.drain(Duration::from_millis(20)));
    assert_eq!(stream.snapshot.finished_reasoning_ids.len(), 1);
    assert_eq!(stream.snapshot.request.thinking_duration_ms, Some(20));
    assert!(!stream.drain(Duration::from_millis(30)));
    assert!(stream.snapshot.finished_reasoning_ids.is_empty());
    sender.try_send(GenerationEvent::Completed).unwrap();
    assert!(stream.drain(Duration::from_millis(40)));
    assert_eq!(stream.snapshot.response.output_text(), "answer");
    assert_eq!(stream.snapshot.request.status, RequestStatus::Completed);
    assert_eq!(stream.snapshot.request.duration_ms, Some(40));
    assert!(stream.snapshot.terminal);
}

#[test]
fn buffered_events_are_consumed_before_an_unexpected_close() {
    let (sender, mut stream) = stream();
    sender.try_send(GenerationEvent::Started).unwrap();
    sender
        .try_send(GenerationEvent::TextDelta("partial".into()))
        .unwrap();
    drop(sender);
    assert!(stream.drain(Duration::from_millis(20)));
    assert_eq!(stream.snapshot.response.output_text(), "partial");
    assert_eq!(stream.snapshot.response.status, MessageStatus::Failed);
    assert_eq!(
        stream.snapshot.request.error.as_ref().unwrap().kind,
        GenerationErrorKind::StreamInterrupted.as_str()
    );
    assert!(stream.snapshot.terminal);
    assert!(!stream.drain(Duration::from_secs(1)));
    assert_eq!(stream.snapshot.request.duration_ms, Some(20));
}

#[test]
fn completion_survives_channel_close_and_trailing_events() {
    let (sender, mut stream) = stream();
    sender
        .try_send(GenerationEvent::TextDelta("answer".into()))
        .unwrap();
    sender.try_send(GenerationEvent::Completed).unwrap();
    sender
        .try_send(GenerationEvent::TextDelta("too late".into()))
        .unwrap();
    drop(sender);
    assert!(stream.drain(Duration::from_millis(10)));
    assert_eq!(stream.snapshot.response.output_text(), "answer");
    assert_eq!(stream.snapshot.request.status, RequestStatus::Completed);
    assert!(stream.snapshot.request.error.is_none());
    assert!(!stream.drain(Duration::from_millis(20)));
}

#[test]
fn cancellation_finishes_reasoning_and_keeps_partial_output_without_an_error() {
    let (sender, mut stream) = stream();
    sender
        .try_send(GenerationEvent::TextDelta("partial".into()))
        .unwrap();
    sender
        .try_send(GenerationEvent::ThinkingDelta {
            provider_id: Some("reasoning".into()),
            delta: "more reasoning".into(),
        })
        .unwrap();
    sender
        .try_send(GenerationEvent::Failed(GenerationError::cancelled()))
        .unwrap();
    assert!(stream.drain(Duration::from_millis(20)));
    assert_eq!(stream.snapshot.response.output_text(), "partial");
    assert_eq!(stream.snapshot.response.status, MessageStatus::Stopped);
    assert_eq!(stream.snapshot.request.status, RequestStatus::Stopped);
    assert!(stream.snapshot.request.error.is_none());
    assert_eq!(stream.snapshot.finished_reasoning_ids.len(), 1);
    assert!(stream.snapshot.terminal);
}

#[test]
fn continuation_normalization_spans_batches_and_preserves_the_transcript() {
    let (sender, mut stream) = stream();
    let prefill = Message::assistant("prefix");
    stream.snapshot.response.append_output("prefix", 0);
    stream.snapshot.response.transcript.push(prefill.clone());
    let mut stream = stream.with_continuation(Some(&prefill));
    sender
        .try_send(GenerationEvent::TextDelta("pre".into()))
        .unwrap();
    assert!(stream.drain(Duration::from_millis(10)));
    assert_eq!(stream.snapshot.response.output_text(), "prefix");
    sender
        .try_send(GenerationEvent::TextDelta("fix suffix".into()))
        .unwrap();
    sender
        .try_send(GenerationEvent::TranscriptContinued(Box::new(
            Message::assistant("prefix suffix"),
        )))
        .unwrap();
    sender.try_send(GenerationEvent::Completed).unwrap();
    assert!(stream.drain(Duration::from_millis(20)));
    assert_eq!(stream.snapshot.response.output_text(), "prefix suffix");
    assert_eq!(
        stream.snapshot.response.transcript,
        vec![Message::assistant("prefix suffix")]
    );
    assert!(stream.snapshot.terminal);
}
