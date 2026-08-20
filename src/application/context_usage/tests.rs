use rig_core::{
    completion::AssistantContent,
    message::{AudioMediaType, Reasoning, UserContent},
};

use super::*;

#[test]
fn provider_usage_anchors_only_the_known_request_and_estimates_the_delta() {
    let previous = vec![Message::user("a".repeat(400))];
    let mut current = previous.clone();
    current.push(Message::user("b".repeat(200)));
    let previous_estimate = estimate_input_tokens("", &previous, 0);
    let current_estimate = estimate_input_tokens("", &current, 0);

    let usage = project_context_usage(
        "",
        &current,
        0,
        Some(1_000),
        Some(ContextUsageReference {
            input_tokens: 300,
            estimated_input_tokens: previous_estimate,
        }),
    );

    assert_eq!(
        usage.input_tokens,
        300 + current_estimate - previous_estimate
    );
    assert_eq!(usage.source, ContextUsageSource::ProviderAnchored);
}

#[test]
fn legacy_single_step_usage_can_build_a_provider_reference() {
    let messages = vec![Message::user("hello")];
    let reference = provider_usage_reference(12, "", &messages, 0).unwrap();
    assert_eq!(reference.input_tokens, 12);
    assert_eq!(
        reference.estimated_input_tokens,
        estimate_input_tokens("", &messages, 0)
    );
    assert!(provider_usage_reference(0, "", &messages, 0).is_none());
}

#[test]
fn reasoning_counts_only_when_it_is_replayed_in_the_transcript() {
    let reasoning = Message::Assistant {
        id: None,
        content: vec![AssistantContent::Reasoning(Reasoning::new("thinking"))],
    };
    let usage = project_context_usage("", &[reasoning], 0, None, None);
    assert!(usage.replays_reasoning);

    let visible_only = project_context_usage("", &[Message::assistant("answer")], 0, None, None);
    assert!(!visible_only.replays_reasoning);
}

#[test]
fn unknown_window_keeps_absolute_usage_without_inventing_a_ratio() {
    let usage = project_context_usage("system", &[Message::user("hello")], 0, None, None);
    assert!(usage.input_tokens > 0);
    assert_eq!(usage.remaining_tokens, None);
    assert_eq!(usage.remaining_ratio, None);
    assert_eq!(usage.source, ContextUsageSource::Estimated);
}

#[test]
fn usage_over_the_window_saturates_remaining_capacity_at_zero() {
    let usage = project_context_usage("", &[Message::user("x".repeat(1_000))], 0, Some(10), None);
    assert_eq!(usage.remaining_tokens, Some(0));
    assert_eq!(usage.remaining_ratio, Some(0.0));
}

#[test]
fn audio_estimate_uses_duration_instead_of_base64_size() {
    let message = |data: String| Message::User {
        content: vec![UserContent::audio(data, Some(AudioMediaType::WAV))],
    };
    let small = message("YQ==".into());
    let large = message("YQ==".repeat(100_000));

    assert_eq!(
        estimate_input_tokens("", &[small], 10_000),
        estimate_input_tokens("", &[large], 10_000)
    );
}

#[test]
fn audio_estimate_grows_at_thirty_two_tokens_per_second() {
    let message = Message::User {
        content: vec![UserContent::audio("YQ==", Some(AudioMediaType::WAV))],
    };
    let baseline = estimate_input_tokens("", std::slice::from_ref(&message), 0);
    assert_eq!(
        estimate_input_tokens("", std::slice::from_ref(&message), 1_000) - baseline,
        32
    );
    assert_eq!(estimate_input_tokens("", &[message], 2_500) - baseline, 80);
}

#[test]
fn provider_anchor_projects_audio_duration_delta() {
    let messages = vec![Message::user("same text")];
    let previous_estimate = estimate_input_tokens("", &messages, 1_000);
    let usage = project_context_usage(
        "",
        &messages,
        3_000,
        None,
        Some(ContextUsageReference {
            input_tokens: 100,
            estimated_input_tokens: previous_estimate,
        }),
    );

    assert_eq!(usage.input_tokens, 164);
    assert_eq!(usage.source, ContextUsageSource::ProviderAnchored);
}
