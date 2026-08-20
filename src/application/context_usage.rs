use rig_core::completion::{AssistantContent, Message};

mod estimate;

pub use estimate::estimate_input_tokens;

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

pub fn context_usage_from_input_tokens(
    input_tokens: u64,
    messages: &[Message],
    context_window_tokens: Option<u32>,
    source: ContextUsageSource,
) -> ContextUsage {
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
    context_usage_from_input_tokens(input_tokens, messages, context_window_tokens, source)
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
mod tests;
