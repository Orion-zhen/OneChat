use crate::{
    application::context_usage::InputEstimate,
    domain::{GenerationError, GenerationErrorKind, Message, Model, RequestContextInfo},
};

use super::history::turn_count;

#[derive(Clone, Copy, Default)]
pub(super) struct InputRequirements {
    pub(super) vision: bool,
    pub(super) audio: bool,
}

#[derive(Clone)]
pub(super) struct MessageGroup {
    messages: Vec<Message>,
    estimate: InputEstimate,
    requirements: InputRequirements,
}

impl MessageGroup {
    pub(super) fn new(
        messages: Vec<Message>,
        audio_duration_ms: u64,
        requirements: InputRequirements,
    ) -> Self {
        let estimate = InputEstimate::new(&messages, audio_duration_ms);
        Self {
            messages,
            estimate,
            requirements,
        }
    }

    pub(super) fn append_transcript(&mut self, messages: Vec<Message>) {
        self.estimate = self.estimate.combine(InputEstimate::new(&messages, 0));
        self.messages.extend(messages);
    }
}

#[derive(Clone)]
pub(super) struct PreparedContext {
    pub(super) opening: Option<MessageGroup>,
    pub(super) history: Vec<MessageGroup>,
    pub(super) current: MessageGroup,
    pub(super) request_context: RequestContextInfo,
}

impl PreparedContext {
    pub(super) fn set_opening(&mut self, content: String) {
        self.opening = Some(MessageGroup::new(
            vec![Message::assistant(content)],
            0,
            InputRequirements::default(),
        ));
    }

    fn fixed_estimate(&self) -> InputEstimate {
        self.current.estimate.combine(
            self.opening
                .as_ref()
                .map_or(InputEstimate::default(), |opening| opening.estimate),
        )
    }

    pub(super) fn estimate(&self) -> InputEstimate {
        self.history
            .iter()
            .fold(self.fixed_estimate(), |total, group| {
                total.combine(group.estimate)
            })
    }

    pub(super) fn into_messages(self) -> Vec<Message> {
        self.opening
            .into_iter()
            .chain(self.history)
            .chain(std::iter::once(self.current))
            .flat_map(|group| group.messages)
            .collect()
    }

    pub(super) fn trim_to_window(&mut self, system_prompt: &str, window: Option<u32>) {
        let Some(window) = window else { return };
        let mut estimate = self.fixed_estimate();
        let mut keep = 0;
        for group in self.history.iter().rev() {
            let candidate = estimate.combine(group.estimate);
            if candidate.tokens(system_prompt) > u64::from(window) {
                break;
            }
            estimate = candidate;
            keep += 1;
        }
        let remove = self.history.len() - keep;
        self.history.drain(..remove);
        self.request_context.included_history_turns = turn_count(keep);
        self.request_context.limited_by_context_window |= remove > 0;
    }

    pub(super) fn validate(&self, model: &Model) -> Result<(), GenerationError> {
        self.check_requirement(
            model.capabilities.vision,
            |input| input.vision,
            "an image or PDF",
        )?;
        self.check_requirement(model.capabilities.audio_input, |input| input.audio, "audio")
    }

    fn check_requirement(
        &self,
        supported: bool,
        required: impl Fn(InputRequirements) -> bool,
        content: &str,
    ) -> Result<(), GenerationError> {
        if supported {
            return Ok(());
        }
        let current = required(self.current.requirements);
        let retained_history = self
            .history
            .iter()
            .any(|group| required(group.requirements));
        if !current && !retained_history {
            return Ok(());
        }
        let location = if current {
            "the current message"
        } else {
            "the retained conversation context"
        };
        Err(GenerationError::new(
            GenerationErrorKind::UnsupportedParameter,
            format!("The selected model cannot read {content} in {location}"),
        ))
    }
}
