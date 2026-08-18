use crate::domain::{GenerationEvent, Message};
use rig_core::completion::AssistantContent;

pub(super) struct ContinuationNormalizer {
    text: PrefixFilter,
    reasoning: PrefixFilter,
    reasoning_provider_id: Option<Option<String>>,
    active: bool,
}

impl ContinuationNormalizer {
    pub(super) fn new(prefill: Option<&Message>) -> Self {
        let (text, reasoning) = prefill.map_or_else(Default::default, assistant_text);
        Self {
            text: PrefixFilter::new(text),
            reasoning: PrefixFilter::new(reasoning),
            reasoning_provider_id: None,
            active: true,
        }
    }

    pub(super) fn normalize(&mut self, event: GenerationEvent) -> Vec<GenerationEvent> {
        if !self.active {
            return vec![event];
        }
        match event {
            GenerationEvent::TextDelta(delta) => {
                let mut events = self.flush_reasoning();
                if let Some(delta) = self.text.push(delta) {
                    events.push(GenerationEvent::TextDelta(delta));
                }
                events
            }
            GenerationEvent::ThinkingDelta { provider_id, delta } => {
                let mut events = self.flush_text();
                if self.reasoning.has_pending()
                    && self.reasoning_provider_id.as_ref() != Some(&provider_id)
                {
                    events.extend(self.flush_reasoning());
                }
                let output_provider_id = self
                    .reasoning_provider_id
                    .clone()
                    .unwrap_or_else(|| provider_id.clone());
                if !self.reasoning.has_pending() {
                    self.reasoning_provider_id = Some(provider_id);
                }
                if let Some(delta) = self.reasoning.push(delta) {
                    events.push(GenerationEvent::ThinkingDelta {
                        provider_id: output_provider_id,
                        delta,
                    });
                }
                if !self.reasoning.has_pending() {
                    self.reasoning_provider_id = None;
                }
                events
            }
            GenerationEvent::TranscriptContinued(_) => {
                let mut events = self.flush_content();
                events.push(event);
                self.active = false;
                events
            }
            GenerationEvent::ToolCallObserved { .. }
            | GenerationEvent::TranscriptAppended(_)
            | GenerationEvent::Completed
            | GenerationEvent::Failed(_) => {
                let mut events = self.flush_content();
                events.push(event);
                if matches!(
                    events.last(),
                    Some(GenerationEvent::Completed | GenerationEvent::Failed(_))
                ) {
                    self.active = false;
                }
                events
            }
            _ => vec![event],
        }
    }

    fn flush_content(&mut self) -> Vec<GenerationEvent> {
        let mut events = self.flush_reasoning();
        events.extend(self.flush_text());
        events
    }

    fn flush_text(&mut self) -> Vec<GenerationEvent> {
        self.text
            .finish()
            .map(GenerationEvent::TextDelta)
            .into_iter()
            .collect()
    }

    fn flush_reasoning(&mut self) -> Vec<GenerationEvent> {
        let provider_id = self.reasoning_provider_id.take().flatten();
        self.reasoning
            .finish()
            .map(|delta| GenerationEvent::ThinkingDelta { provider_id, delta })
            .into_iter()
            .collect()
    }
}

#[derive(Default)]
struct PrefixFilter {
    prefix: String,
    pending: String,
    replayed: bool,
    passthrough: bool,
}

impl PrefixFilter {
    fn new(prefix: String) -> Self {
        Self {
            passthrough: prefix.is_empty(),
            prefix,
            ..Self::default()
        }
    }

    fn push(&mut self, delta: String) -> Option<String> {
        if self.replayed || self.passthrough {
            return (!delta.is_empty()).then_some(delta);
        }
        self.pending.push_str(&delta);
        if self.prefix.starts_with(&self.pending) {
            if self.pending == self.prefix {
                self.pending.clear();
                self.replayed = true;
            }
            return None;
        }
        if self.pending.starts_with(&self.prefix) {
            let suffix = self.pending.split_off(self.prefix.len());
            self.pending.clear();
            self.replayed = true;
            return (!suffix.is_empty()).then_some(suffix);
        }
        self.passthrough = true;
        Some(std::mem::take(&mut self.pending))
    }

    fn finish(&mut self) -> Option<String> {
        if self.pending.is_empty() {
            return None;
        }
        self.passthrough = true;
        Some(std::mem::take(&mut self.pending))
    }

    fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}

fn assistant_text(message: &Message) -> (String, String) {
    let Message::Assistant { content, .. } = message else {
        return Default::default();
    };
    let mut text = String::new();
    let mut reasoning = String::new();
    for item in content.iter() {
        match item {
            AssistantContent::Text(item) => text.push_str(&item.text),
            AssistantContent::Reasoning(item) => reasoning.push_str(&item.display_text()),
            _ => {}
        }
    }
    (text, reasoning)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig_core::message::Reasoning;

    fn prefill() -> Message {
        Message::Assistant {
            id: None,
            content: vec![
                AssistantContent::Reasoning(Reasoning::new("old reasoning")),
                AssistantContent::text("old answer"),
            ],
        }
    }

    #[test]
    fn removes_replayed_prefill_from_streams() {
        let message = prefill();
        let mut normalizer = ContinuationNormalizer::new(Some(&message));
        let events = [
            GenerationEvent::ThinkingDelta {
                provider_id: None,
                delta: "old ".into(),
            },
            GenerationEvent::ThinkingDelta {
                provider_id: None,
                delta: "reasoning".into(),
            },
            GenerationEvent::TextDelta("old ans".into()),
            GenerationEvent::TextDelta("wer continued".into()),
        ]
        .into_iter()
        .flat_map(|event| normalizer.normalize(event))
        .collect::<Vec<_>>();

        assert_eq!(
            events,
            vec![GenerationEvent::TextDelta(" continued".into())]
        );
    }

    #[test]
    fn preserves_suffix_only_streams() {
        let message = prefill();
        let mut normalizer = ContinuationNormalizer::new(Some(&message));
        assert_eq!(
            normalizer.normalize(GenerationEvent::ThinkingDelta {
                provider_id: None,
                delta: "new reasoning".into(),
            }),
            vec![GenerationEvent::ThinkingDelta {
                provider_id: None,
                delta: "new reasoning".into(),
            }]
        );
        assert_eq!(
            normalizer.normalize(GenerationEvent::TextDelta(" continued".into())),
            vec![GenerationEvent::TextDelta(" continued".into())]
        );
    }
}
