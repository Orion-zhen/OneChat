use rig_core::{
    OneOrMany,
    completion::{AssistantContent, Message},
    message::{Audio, DocumentSourceKind, UserContent},
};

const AUDIO_INPUT_TOKENS_PER_SECOND: u64 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextUsageSource {
    Estimated,
    ProviderAnchored,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextUsageReference {
    pub input_tokens: u64,
    pub estimated_input_tokens: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContextUsage {
    pub input_tokens: u64,
    pub context_window_tokens: Option<u32>,
    pub remaining_tokens: Option<u64>,
    pub remaining_ratio: Option<f32>,
    pub source: ContextUsageSource,
    pub replays_reasoning: bool,
}

pub fn estimate_input_tokens(
    system_prompt: &str,
    messages: &[Message],
    audio_duration_ms: u64,
) -> u64 {
    let characters = system_prompt.chars().count()
        + messages
            .iter()
            .map(message_characters_without_audio_data)
            .sum::<usize>();
    let text_tokens = characters.div_ceil(4) as u64;
    let audio_tokens = audio_duration_ms
        .saturating_mul(AUDIO_INPUT_TOKENS_PER_SECOND)
        .div_ceil(1_000);
    text_tokens.saturating_add(audio_tokens)
}

fn message_characters_without_audio_data(message: &Message) -> usize {
    let Message::User { content } = message else {
        return serialized_characters(message);
    };
    if !content
        .iter()
        .any(|content| matches!(content, UserContent::Audio(_)))
    {
        return serialized_characters(message);
    }

    let content = content
        .iter()
        .map(|content| match content {
            UserContent::Audio(audio) => UserContent::Audio(Audio {
                data: DocumentSourceKind::Unknown,
                media_type: audio.media_type.clone(),
                additional_params: audio.additional_params.clone(),
            }),
            content => content.clone(),
        })
        .collect::<Vec<_>>();
    let message = Message::User {
        content: OneOrMany::many(content).expect("user messages have content"),
    };
    serialized_characters(&message)
}

fn serialized_characters(value: &impl serde::Serialize) -> usize {
    serde_json::to_string(value).map_or(0, |value| value.chars().count())
}

pub fn provider_usage_reference(
    input_tokens: u64,
    system_prompt: &str,
    messages: &[Message],
    audio_duration_ms: u64,
) -> Option<ContextUsageReference> {
    let estimated_input_tokens = estimate_input_tokens(system_prompt, messages, audio_duration_ms);
    (input_tokens > 0 && estimated_input_tokens > 0).then_some(ContextUsageReference {
        input_tokens,
        estimated_input_tokens,
    })
}

pub fn project_context_usage(
    system_prompt: &str,
    messages: &[Message],
    audio_duration_ms: u64,
    context_window_tokens: Option<u32>,
    reference: Option<ContextUsageReference>,
) -> ContextUsage {
    let estimated_input_tokens = estimate_input_tokens(system_prompt, messages, audio_duration_ms);
    let (input_tokens, source) = reference
        .filter(|reference| reference.input_tokens > 0 && reference.estimated_input_tokens > 0)
        .map_or(
            (estimated_input_tokens, ContextUsageSource::Estimated),
            |reference| {
                let delta = i128::from(estimated_input_tokens)
                    - i128::from(reference.estimated_input_tokens);
                let projected = (i128::from(reference.input_tokens) + delta)
                    .clamp(0, i128::from(u64::MAX)) as u64;
                (projected, ContextUsageSource::ProviderAnchored)
            },
        );
    let (remaining_tokens, remaining_ratio) =
        context_window_tokens.map_or((None, None), |window| {
            let window = u64::from(window);
            let remaining = window.saturating_sub(input_tokens);
            let ratio = if window == 0 {
                0.0
            } else {
                remaining as f32 / window as f32
            };
            (Some(remaining), Some(ratio))
        });

    ContextUsage {
        input_tokens,
        context_window_tokens,
        remaining_tokens,
        remaining_ratio,
        source,
        replays_reasoning: messages.iter().any(message_replays_reasoning),
    }
}

fn message_replays_reasoning(message: &Message) -> bool {
    let Message::Assistant { content, .. } = message else {
        return false;
    };
    content
        .iter()
        .any(|content| matches!(content, AssistantContent::Reasoning(_)))
}

#[cfg(test)]
mod tests {
    use rig_core::{
        OneOrMany,
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
            content: OneOrMany::one(AssistantContent::Reasoning(Reasoning::new("thinking"))),
        };
        let usage = project_context_usage("", &[reasoning], 0, None, None);
        assert!(usage.replays_reasoning);

        let visible_only =
            project_context_usage("", &[Message::assistant("answer")], 0, None, None);
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
        let usage =
            project_context_usage("", &[Message::user("x".repeat(1_000))], 0, Some(10), None);
        assert_eq!(usage.remaining_tokens, Some(0));
        assert_eq!(usage.remaining_ratio, Some(0.0));
    }

    #[test]
    fn audio_estimate_uses_duration_instead_of_base64_size() {
        let message = |data: String| Message::User {
            content: OneOrMany::one(UserContent::audio(data, Some(AudioMediaType::WAV))),
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
            content: OneOrMany::one(UserContent::audio("YQ==", Some(AudioMediaType::WAV))),
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
}
