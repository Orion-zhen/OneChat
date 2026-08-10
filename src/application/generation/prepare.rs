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

#[derive(Clone)]
pub(super) struct PreparedHistoryGroup {
    pub(super) message_count: usize,
    pub(super) requires_vision: bool,
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
    pub(super) current_message_requires_vision: bool,
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
            &conversation.system_prompt,
            &context.messages,
        );
        request_info.context = Some(context.request_context);
        let response = response.clone();
        let provider_request = provider_request(
            provider,
            model,
            &conversation.system_prompt,
            &conversation.generation_config,
            context.messages,
        );
        Ok(Self {
            start: GenerationStart::NewTurn(Box::new(turn)),
            response,
            request_info,
            provider_request,
            tool_selection: conversation.tool_selection.clone(),
            new_attachments: Vec::new(),
            history_groups: context.history_groups,
            current_message_requires_vision: context.current_message_requires_vision,
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

        if !self.provider_request.model.capabilities.vision {
            let retained_history_requires_vision = self
                .history_groups
                .iter()
                .any(|group| group.requires_vision);
            if self.current_message_requires_vision || retained_history_requires_vision {
                let message = if self.current_message_requires_vision {
                    "The selected model cannot read an image or PDF in the current message"
                } else {
                    "The selected model cannot read an image or PDF in the retained conversation context"
                };
                return Err(GenerationError::new(
                    GenerationErrorKind::UnsupportedParameter,
                    message,
                ));
            }
        }

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
            &conversation.system_prompt,
            &context.messages,
        );
        request_info.context = Some(context.request_context);
        let provider_request = provider_request(
            provider,
            model,
            &conversation.system_prompt,
            &turn.generation_config,
            context.messages,
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
            current_message_requires_vision: context.current_message_requires_vision,
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
            &conversation.system_prompt,
            &context.messages,
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
            current_message_requires_vision: context.current_message_requires_vision,
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

#[derive(Clone, Copy)]
struct AncestorTurn<'a> {
    turn: &'a Turn,
    response: &'a AssistantResponse,
}

struct HistorySelection<'a> {
    limit: HistoryLimit,
    available: usize,
    ancestors: Vec<AncestorTurn<'a>>,
}

impl<'a> HistorySelection<'a> {
    fn new(turns: &'a [Turn], response_id: Option<&str>, limit: HistoryLimit) -> Self {
        let limit = limit.normalized();
        let mut ancestors = response_id
            .map(|response_id| lineage_through_response(turns, response_id))
            .unwrap_or_default();
        let available = ancestors.len();
        if let HistoryLimit::Last(turns) = limit {
            let keep = usize::try_from(turns).unwrap_or(usize::MAX);
            let remove = ancestors.len().saturating_sub(keep);
            ancestors.drain(..remove);
        }
        Self {
            limit,
            available,
            ancestors,
        }
    }

    fn request_context(&self) -> RequestContextInfo {
        RequestContextInfo {
            history_limit: self.limit,
            available_history_turns: turn_count(self.available),
            included_history_turns: turn_count(self.ancestors.len()),
            limited_by_context_window: false,
        }
    }
}

struct PreparedContext {
    messages: Vec<Message>,
    history_groups: Vec<PreparedHistoryGroup>,
    current_message_requires_vision: bool,
    request_context: RequestContextInfo,
}

fn prepare_context(
    turns: &[Turn],
    parent_response_id: Option<&str>,
    current_user: &UserMessage,
    history_limit: HistoryLimit,
    user_message: &dyn Fn(&UserMessage) -> Result<Message, String>,
) -> Result<PreparedContext, String> {
    let selection = HistorySelection::new(turns, parent_response_id, history_limit);
    let current_message_requires_vision = user_requires_vision(current_user);

    let mut messages = Vec::new();
    let mut history_groups = Vec::with_capacity(selection.ancestors.len());
    for ancestor in &selection.ancestors {
        let start = messages.len();
        expand_ancestor(*ancestor, user_message, &mut messages)?;
        history_groups.push(PreparedHistoryGroup {
            message_count: messages.len() - start,
            requires_vision: user_requires_vision(&ancestor.turn.user),
        });
    }
    messages.push(user_message(current_user)?);

    Ok(PreparedContext {
        messages,
        history_groups,
        current_message_requires_vision,
        request_context: selection.request_context(),
    })
}

pub fn history_for_turn(turns: &[Turn], turn: &Turn, limit: HistoryLimit) -> Vec<Message> {
    history_for_turn_with(turns, turn, limit, &plain_user_message).unwrap_or_default()
}

fn history_for_turn_with(
    turns: &[Turn],
    turn: &Turn,
    limit: HistoryLimit,
    user_message: &dyn Fn(&UserMessage) -> Result<Message, String>,
) -> Result<Vec<Message>, String> {
    let selection = HistorySelection::new(turns, turn.parent_response_id.as_deref(), limit);
    let mut messages = expand_selection(&selection, user_message)?;
    messages.push(user_message(&turn.user)?);
    Ok(messages)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HistoryPreview {
    pub available_turns: u32,
    pub included_turns: u32,
}

pub fn history_for_new_turn(turns: &[Turn], limit: HistoryLimit) -> Vec<Message> {
    let selection = history_selection_for_new_turn(turns, limit);
    expand_selection(&selection, &plain_user_message).unwrap_or_default()
}

pub fn history_preview_for_new_turn(turns: &[Turn], limit: HistoryLimit) -> HistoryPreview {
    let selection = history_selection_for_new_turn(turns, limit);
    HistoryPreview {
        available_turns: turn_count(selection.available),
        included_turns: turn_count(selection.ancestors.len()),
    }
}

fn history_selection_for_new_turn(turns: &[Turn], limit: HistoryLimit) -> HistorySelection<'_> {
    let response_id = active_turns(turns)
        .last()
        .and_then(|turn| turn.continuation_response_id.as_deref());
    HistorySelection::new(turns, response_id, limit)
}

fn plain_user_message(user: &UserMessage) -> Result<Message, String> {
    Ok(Message::user(user.content.clone()))
}

fn lineage_through_response<'a>(turns: &'a [Turn], response_id: &str) -> Vec<AncestorTurn<'a>> {
    let mut lineage = Vec::new();
    let mut visited = HashSet::new();
    let mut response_id = Some(response_id);

    while let Some(current_response_id) = response_id {
        if !visited.insert(current_response_id.to_string()) {
            break;
        }
        let Some((turn, response)) = turns.iter().find_map(|turn| {
            turn.response(current_response_id)
                .map(|response| (turn, response))
        }) else {
            break;
        };
        lineage.push(AncestorTurn { turn, response });
        response_id = turn.parent_response_id.as_deref();
    }

    lineage.reverse();
    lineage
}

fn expand_selection(
    selection: &HistorySelection<'_>,
    user_message: &dyn Fn(&UserMessage) -> Result<Message, String>,
) -> Result<Vec<Message>, String> {
    let mut messages = Vec::new();
    for ancestor in &selection.ancestors {
        expand_ancestor(*ancestor, user_message, &mut messages)?;
    }
    Ok(messages)
}

fn expand_ancestor(
    ancestor: AncestorTurn<'_>,
    user_message: &dyn Fn(&UserMessage) -> Result<Message, String>,
    messages: &mut Vec<Message>,
) -> Result<(), String> {
    messages.push(user_message(&ancestor.turn.user)?);
    if ancestor.response.transcript.is_empty() {
        if !ancestor.response.content.is_empty() {
            messages.push(Message::assistant(ancestor.response.content.clone()));
        }
    } else {
        messages.extend(ancestor.response.transcript.clone());
    }
    Ok(())
}

fn user_requires_vision(user: &UserMessage) -> bool {
    user.attachments
        .iter()
        .any(|attachment| attachment.kind.requires_vision())
}

fn turn_count(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

fn prepare_response(
    conversation_id: &str,
    turn_id: &str,
    response: &mut AssistantResponse,
    provider: &Provider,
    model: &Model,
    system_prompt: &str,
    messages: &[Message],
) -> RequestInfo {
    response.status = MessageStatus::Streaming;
    response.content.clear();
    response.thinking.clear();
    response.transcript.clear();
    response.tool_executions.clear();
    response.updated_at = now_timestamp();
    let mut request = RequestInfo::new(conversation_id, turn_id, &response.id);
    request.provider_id = Some(provider.id.clone());
    request.model_id = Some(model.id.clone());
    response.request_id = Some(request.id.clone());

    request.usage.input_tokens = Some(estimate_input_tokens(system_prompt, messages));
    request.usage.estimated = true;

    request
}

fn update_input_token_estimate(request: &mut RequestInfo, provider_request: &GenerationRequest) {
    request.usage.input_tokens = Some(estimate_input_tokens(
        &provider_request.system_prompt,
        &provider_request.messages,
    ));
    request.usage.estimated = true;
}

fn provider_request(
    provider: &Provider,
    model: &Model,
    system_prompt: &str,
    config: &GenerationConfig,
    messages: Vec<Message>,
) -> GenerationRequest {
    let (config, _) = config.filtered_for(&model.capabilities);
    GenerationRequest {
        provider: provider.clone(),
        model: model.clone(),
        system_prompt: system_prompt.to_string(),
        config,
        messages,
        tools: Vec::new(),
    }
}
