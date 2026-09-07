use std::collections::{BTreeMap, HashSet};

use tokio_util::sync::CancellationToken;

use crate::{
    application::prompt::{PromptContext, PromptRenderError, render_prompt_templates},
    domain::{
        AssistantResponse, Attachment, Conversation, GenerationConfig, GenerationError,
        GenerationRequest, HistoryLimit, Message, MessageStatus, Model, PromptVariableSource,
        Provider, RequestContextInfo, RequestInfo, RequestKind, ToolSelection, Turn, UserMessage,
        active_turns, now_timestamp,
    },
};
mod context;
mod history;
mod request;

use history::prepare_context;
pub use history::{
    HistoryPreview, history_audio_duration_ms_for_new_turn, history_audio_duration_ms_for_turn,
    history_for_new_turn, history_for_turn, history_preview_for_new_turn,
};
pub use request::PreparedRequest;
use request::prepare_response;

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

pub use crate::domain::GenerationStart;

#[derive(Clone)]
pub struct PreparedGeneration {
    pub start: GenerationStart,
    pub response: AssistantResponse,
    pub request_info: RequestInfo,
    pub request: PreparedRequest,
    pub tool_selection: ToolSelection,
    pub new_attachments: Vec<Attachment>,
    pub continuation_baseline: Option<AssistantResponse>,
    assistant_opening_template: Option<String>,
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
        let request = PreparedRequest::new(
            conversation,
            provider,
            model,
            &conversation.generation_config,
            context,
        );
        let response = &mut turn.responses[0];
        let request_info = prepare_response(
            &conversation.id,
            &turn.id,
            response,
            RequestKind::Generate,
            &request,
        );
        let response = response.clone();
        Ok(Self::assembled(
            conversation,
            GenerationStart::NewTurn(Box::new(turn)),
            response,
            request_info,
            request,
            None,
        ))
    }

    fn assembled(
        conversation: &Conversation,
        start: GenerationStart,
        response: AssistantResponse,
        request_info: RequestInfo,
        request: PreparedRequest,
        continuation_baseline: Option<AssistantResponse>,
    ) -> Self {
        Self {
            start,
            response,
            request_info,
            request,
            tool_selection: conversation.tool_selection.clone(),
            new_attachments: Vec::new(),
            continuation_baseline,
            assistant_opening_template: (!conversation.assistant_opening.is_empty())
                .then(|| conversation.assistant_opening.clone()),
            prompt_variables: BTreeMap::new(),
            prompt_context: PromptContext::default(),
        }
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

    pub async fn render_prompt_setup(
        &mut self,
        cancellation: CancellationToken,
    ) -> Result<(), PromptRenderError> {
        let system_template = self.request.system_prompt.clone();
        let mut templates = vec![system_template.clone()];
        templates.extend(self.assistant_opening_template.clone());
        let snapshots = render_prompt_templates(
            templates,
            std::mem::take(&mut self.prompt_variables),
            std::mem::take(&mut self.prompt_context),
            cancellation,
        )
        .await;
        let mut snapshots = match snapshots {
            Ok(snapshots) => snapshots,
            Err(error) => {
                self.request_info.system_prompt = Some(crate::domain::PromptSnapshot {
                    template: system_template,
                    ..Default::default()
                });
                self.request_info.assistant_opening =
                    self.assistant_opening_template.clone().map(|template| {
                        crate::domain::PromptSnapshot {
                            template,
                            ..Default::default()
                        }
                    });
                return Err(error);
            }
        };
        let system_snapshot = snapshots.remove(0);
        self.request.system_prompt = system_snapshot.resolved.clone();
        self.request_info.system_prompt = Some(system_snapshot);
        if let Some(opening_snapshot) = snapshots.pop() {
            self.request
                .context
                .set_opening(opening_snapshot.resolved.clone());
            self.request_info.assistant_opening = Some(opening_snapshot);
        }
        self.request.update_request_info(&mut self.request_info);
        Ok(())
    }

    pub fn finalize_context(&mut self) -> Result<(), GenerationError> {
        self.request.context.trim_to_window(
            &self.request.system_prompt,
            self.request.model.context_window_tokens,
        );
        self.request.update_request_info(&mut self.request_info);
        self.request.context.validate(&self.request.model)
    }

    pub fn additional(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        turns: &[Turn],
        turn: &Turn,
        context_policy: ContextPolicy<'_>,
    ) -> Result<Self, String> {
        Self::existing_turn(
            (conversation, provider, model),
            turns,
            turn,
            AssistantResponse::new(model, provider),
            turn.generation_config.clone(),
            GenerationStart::AddResponse {
                turn_id: turn.id.clone(),
            },
            context_policy,
        )
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
        let mut config = turn.generation_config.clone();
        config
            .reasoning_preset
            .clone_from(&conversation.generation_config.reasoning_preset);
        Self::existing_turn(
            (conversation, provider, model),
            turns,
            turn,
            previous_response.clone(),
            config,
            GenerationStart::RetryResponse {
                turn_id: turn.id.clone(),
            },
            context_policy,
        )
    }

    pub fn continuation(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        turns: &[Turn],
        turn: &Turn,
        previous_response: &AssistantResponse,
        context_policy: ContextPolicy<'_>,
    ) -> Result<Self, String> {
        if !previous_response.has_output() {
            return Err("Only a response with output can be continued".into());
        }
        let mut response = previous_response.clone();
        response.prepare_continuation();
        let mut context = prepare_context(
            turns,
            turn.parent_response_id.as_deref(),
            &turn.user,
            context_policy.history_limit,
            context_policy.user_message,
        )?;
        if response.transcript.is_empty() {
            return Err("The response has no assistant transcript to continue".into());
        }
        context
            .current
            .append_transcript(response.transcript.clone());
        let mut config = turn.generation_config.clone();
        config
            .reasoning_preset
            .clone_from(&conversation.generation_config.reasoning_preset);
        let request = PreparedRequest::new(conversation, provider, model, &config, context);
        let request_info = prepare_response(
            &conversation.id,
            &turn.id,
            &mut response,
            RequestKind::Continue,
            &request,
        );
        Ok(Self::assembled(
            conversation,
            GenerationStart::ContinueResponse {
                turn_id: turn.id.clone(),
            },
            response,
            request_info,
            request,
            Some(previous_response.clone()),
        ))
    }

    fn existing_turn(
        target: (&Conversation, &Provider, &Model),
        turns: &[Turn],
        turn: &Turn,
        mut response: AssistantResponse,
        config: GenerationConfig,
        start: GenerationStart,
        context_policy: ContextPolicy<'_>,
    ) -> Result<Self, String> {
        let (conversation, provider, model) = target;
        let context = prepare_context(
            turns,
            turn.parent_response_id.as_deref(),
            &turn.user,
            context_policy.history_limit,
            context_policy.user_message,
        )?;
        let request = PreparedRequest::new(conversation, provider, model, &config, context);
        let kind = match &start {
            GenerationStart::AddResponse { .. } => RequestKind::Additional,
            GenerationStart::RetryResponse { .. } => RequestKind::Regenerate,
            GenerationStart::NewTurn(_) | GenerationStart::ContinueResponse { .. } => {
                unreachable!("existing turn preparation received an invalid start")
            }
        };
        let request_info =
            prepare_response(&conversation.id, &turn.id, &mut response, kind, &request);
        Ok(Self::assembled(
            conversation,
            start,
            response,
            request_info,
            request,
            None,
        ))
    }
}
