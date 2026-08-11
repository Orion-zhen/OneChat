use std::collections::{BTreeMap, HashSet};

use tokio_util::sync::CancellationToken;

use crate::{
    application::{
        context_usage::estimate_input_tokens,
        prompt::{PromptContext, PromptRenderError, render_prompt},
    },
    domain::{
        AssistantResponse, Attachment, Conversation, GenerationConfig, GenerationError,
        GenerationErrorKind, GenerationRequest, HistoryLimit, Message, MessageStatus, Model,
        PromptVariableSource, Provider, RequestContextInfo, RequestInfo, ToolSelection, Turn,
        UserMessage, active_turns, now_timestamp,
    },
};
mod history;
mod request;

pub use history::{
    HistoryPreview, history_audio_duration_ms_for_new_turn, history_audio_duration_ms_for_turn,
    history_for_new_turn, history_for_turn, history_preview_for_new_turn,
};
use history::{prepare_context, turn_count};
use request::{RequestInput, prepare_response, provider_request, update_input_token_estimate};

#[derive(Clone, Copy)]
pub struct ContextPolicy<'a> {
    history_limit: HistoryLimit,
    user_message: &'a dyn Fn(&UserMessage) -> Result<Message, String>,
}

impl<'a> ContextPolicy<'a> {
    pub fn new(
        history_limit: HistoryLimit,
        user_message: &'a dyn Fn(&UserMessage) -> Result<Message, String>,
    ) -> Self {
        Self {
            history_limit,
            user_message,
        }
    }
}

#[derive(Clone)]
pub enum GenerationStart {
    NewTurn(Box<Turn>),
    AddResponse { turn_id: String },
    RetryResponse { turn_id: String },
}

#[derive(Clone, Copy, Default)]
pub(super) struct InputRequirements {
    pub(super) vision: bool,
    pub(super) audio: bool,
}

#[derive(Clone)]
pub(super) struct PreparedHistoryGroup {
    pub(super) message_count: usize,
    pub(super) audio_duration_ms: u64,
    pub(super) requirements: InputRequirements,
}

#[derive(Clone)]
pub struct PreparedGeneration {
    pub start: GenerationStart,
    pub response: AssistantResponse,
    pub request_info: RequestInfo,
    pub provider_request: GenerationRequest,
    pub tool_selection: ToolSelection,
    pub new_attachments: Vec<Attachment>,
    pub(super) history_groups: Vec<PreparedHistoryGroup>,
    pub(super) current_message_requirements: InputRequirements,
    prompt_variables: BTreeMap<String, PromptVariableSource>,
    prompt_context: PromptContext,
}

impl PreparedGeneration {
    pub fn new(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        turns: &[Turn],
        parent_response_id: Option<String>,
        user: UserMessage,
        context_policy: ContextPolicy<'_>,
    ) -> Result<Self, String> {
        let response = AssistantResponse::new(model, provider);
        let mut turn = Turn::new(conversation, parent_response_id.clone(), user, response);
        let context = prepare_context(
            turns,
            parent_response_id.as_deref(),
            &turn.user,
            context_policy.history_limit,
            context_policy.user_message,
        )?;
        let response = &mut turn.responses[0];
        let mut request_info = prepare_response(
            &conversation.id,
            &turn.id,
            response,
            provider,
            model,
            RequestInput::new(
                &conversation.system_prompt,
                &context.messages,
                context.audio_duration_ms,
            ),
        );
        request_info.context = Some(context.request_context);
        let response = response.clone();
        let provider_request = provider_request(
            provider,
            model,
            &conversation.system_prompt,
            &conversation.generation_config,
            context.messages,
            context.audio_duration_ms,
        );
        Ok(Self {
            start: GenerationStart::NewTurn(Box::new(turn)),
            response,
            request_info,
            provider_request,
            tool_selection: conversation.tool_selection.clone(),
            new_attachments: Vec::new(),
            history_groups: context.history_groups,
            current_message_requirements: context.current_message_requirements,
            prompt_variables: BTreeMap::new(),
            prompt_context: PromptContext::default(),
        }
        .validated())
    }

    pub fn with_new_attachments(mut self, attachments: Vec<Attachment>) -> Self {
        self.new_attachments = attachments;
        self
    }

    pub fn configure_prompt(
        &mut self,
        variables: BTreeMap<String, PromptVariableSource>,
        context: PromptContext,
    ) {
        self.prompt_variables = variables;
        self.prompt_context = context;
    }

    pub async fn render_system_prompt(
        &mut self,
        cancellation: CancellationToken,
    ) -> Result<(), PromptRenderError> {
        let template = self.provider_request.system_prompt.clone();
        let snapshot = render_prompt(
            template.clone(),
            std::mem::take(&mut self.prompt_variables),
            std::mem::take(&mut self.prompt_context),
            cancellation,
        )
        .await;
        let snapshot = match snapshot {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.request_info.system_prompt = Some(crate::domain::PromptSnapshot {
                    template,
                    ..Default::default()
                });
                return Err(error);
            }
        };
        self.provider_request.system_prompt = snapshot.resolved.clone();
        self.request_info.system_prompt = Some(snapshot);
        update_input_token_estimate(&mut self.request_info, &self.provider_request);
        Ok(())
    }

    pub fn finalize_context(&mut self) -> Result<(), GenerationError> {
        update_input_token_estimate(&mut self.request_info, &self.provider_request);

        if let Some(context_window) = self.provider_request.model.context_window_tokens {
            while self.estimated_input_tokens() > u64::from(context_window)
                && !self.history_groups.is_empty()
            {
                let group = self.history_groups.remove(0);
                self.provider_request.messages.drain(..group.message_count);
                self.provider_request.audio_duration_ms = self
                    .provider_request
                    .audio_duration_ms
                    .saturating_sub(group.audio_duration_ms);
                if let Some(context) = &mut self.request_info.context {
                    context.included_history_turns = turn_count(self.history_groups.len());
                    context.limited_by_context_window = true;
                }
                update_input_token_estimate(&mut self.request_info, &self.provider_request);
            }

            if self.estimated_input_tokens() > u64::from(context_window) {
                return Err(GenerationError::new(
                    GenerationErrorKind::ContextLengthExceeded,
                    "System prompt and current message exceed the model context window",
                )
                .with_detail(format!(
                    "Estimated {} input tokens for a configured {}-token context window",
                    self.estimated_input_tokens(),
                    context_window
                )));
            }
        }

        let capabilities = &self.provider_request.model.capabilities;
        self.check_input_requirement(
            capabilities.vision,
            |requirements| requirements.vision,
            "an image or PDF",
        )?;
        self.check_input_requirement(
            capabilities.audio_input,
            |requirements| requirements.audio,
            "audio",
        )?;

        debug_assert_eq!(
            self.history_groups
                .iter()
                .map(|group| group.message_count)
                .sum::<usize>()
                + 1,
            self.provider_request.messages.len()
        );
        Ok(())
    }

    fn check_input_requirement(
        &self,
        supported: bool,
        required: impl Fn(InputRequirements) -> bool,
        content: &str,
    ) -> Result<(), GenerationError> {
        if supported {
            return Ok(());
        }
        let current = required(self.current_message_requirements);
        let retained_history = self
            .history_groups
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

    fn estimated_input_tokens(&self) -> u64 {
        self.request_info.usage.input_tokens.unwrap_or_default()
    }

    pub fn additional(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        turns: &[Turn],
        turn: &Turn,
        context_policy: ContextPolicy<'_>,
    ) -> Result<Self, String> {
        let context = prepare_context(
            turns,
            turn.parent_response_id.as_deref(),
            &turn.user,
            context_policy.history_limit,
            context_policy.user_message,
        )?;
        let mut response = AssistantResponse::new(model, provider);
        let mut request_info = prepare_response(
            &conversation.id,
            &turn.id,
            &mut response,
            provider,
            model,
            RequestInput::new(
                &conversation.system_prompt,
                &context.messages,
                context.audio_duration_ms,
            ),
        );
        request_info.context = Some(context.request_context);
        let provider_request = provider_request(
            provider,
            model,
            &conversation.system_prompt,
            &turn.generation_config,
            context.messages,
            context.audio_duration_ms,
        );
        Ok(Self {
            start: GenerationStart::AddResponse {
                turn_id: turn.id.clone(),
            },
            response,
            request_info,
            provider_request,
            tool_selection: conversation.tool_selection.clone(),
            new_attachments: Vec::new(),
            history_groups: context.history_groups,
            current_message_requirements: context.current_message_requirements,
            prompt_variables: BTreeMap::new(),
            prompt_context: PromptContext::default(),
        }
        .validated())
    }

    pub fn regenerate(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        turns: &[Turn],
        turn: &Turn,
        previous_response: &AssistantResponse,
        context_policy: ContextPolicy<'_>,
    ) -> Result<Self, String> {
        let context = prepare_context(
            turns,
            turn.parent_response_id.as_deref(),
            &turn.user,
            context_policy.history_limit,
            context_policy.user_message,
        )?;
        let mut response = previous_response.clone();
        let mut request_info = prepare_response(
            &conversation.id,
            &turn.id,
            &mut response,
            provider,
            model,
            RequestInput::new(
                &conversation.system_prompt,
                &context.messages,
                context.audio_duration_ms,
            ),
        );
        request_info.context = Some(context.request_context);
        let mut config = turn.generation_config.clone();
        config
            .reasoning_preset
            .clone_from(&conversation.generation_config.reasoning_preset);
        let provider_request = provider_request(
            provider,
            model,
            &conversation.system_prompt,
            &config,
            context.messages,
            context.audio_duration_ms,
        );
        Ok(Self {
            start: GenerationStart::RetryResponse {
                turn_id: turn.id.clone(),
            },
            response,
            request_info,
            provider_request,
            tool_selection: conversation.tool_selection.clone(),
            new_attachments: Vec::new(),
            history_groups: context.history_groups,
            current_message_requirements: context.current_message_requirements,
            prompt_variables: BTreeMap::new(),
            prompt_context: PromptContext::default(),
        }
        .validated())
    }

    fn validated(self) -> Self {
        debug_assert_eq!(
            self.history_groups
                .iter()
                .map(|group| group.message_count)
                .sum::<usize>()
                + 1,
            self.provider_request.messages.len()
        );
        self
    }
}
