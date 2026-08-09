use std::collections::HashSet;

use super::reducer::estimate_tokens;

use crate::domain::{
    AssistantResponse, Attachment, Conversation, GenerationConfig, GenerationRequest, Message,
    MessageStatus, Model, Provider, RequestInfo, ToolSelection, Turn, UserMessage, active_turns,
    now_timestamp,
};

#[derive(Clone)]
pub enum GenerationStart {
    NewTurn(Box<Turn>),
    AddResponse { turn_id: String },
    RetryResponse { turn_id: String },
}

#[derive(Clone)]
pub struct PreparedGeneration {
    pub start: GenerationStart,
    pub response: AssistantResponse,
    pub request_info: RequestInfo,
    pub provider_request: GenerationRequest,
    pub tool_selection: ToolSelection,
    pub new_attachments: Vec<Attachment>,
}

impl PreparedGeneration {
    pub fn new(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        turns: &[Turn],
        parent_response_id: Option<String>,
        user: UserMessage,
        user_message: &dyn Fn(&UserMessage) -> Result<Message, String>,
    ) -> Result<Self, String> {
        let response = AssistantResponse::new(model, provider);
        let mut turn = Turn::new(conversation, parent_response_id.clone(), user, response);
        let mut messages = parent_response_id
            .as_deref()
            .map(|response_id| history_through_response(turns, response_id, user_message))
            .transpose()?
            .unwrap_or_default();
        messages.push(user_message(&turn.user)?);
        let response = &mut turn.responses[0];
        let request_info = prepare_response(
            &conversation.id,
            &turn.id,
            response,
            provider,
            model,
            &conversation.system_prompt,
            &messages,
        );
        let response = response.clone();
        let provider_request = provider_request(
            provider,
            model,
            &conversation.system_prompt,
            &conversation.generation_config,
            messages,
        );
        Ok(Self {
            start: GenerationStart::NewTurn(Box::new(turn)),
            response,
            request_info,
            provider_request,
            tool_selection: conversation.tool_selection.clone(),
            new_attachments: Vec::new(),
        })
    }

    pub fn with_new_attachments(mut self, attachments: Vec<Attachment>) -> Self {
        self.new_attachments = attachments;
        self
    }

    pub fn additional(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        turns: &[Turn],
        turn: &Turn,
        user_message: &dyn Fn(&UserMessage) -> Result<Message, String>,
    ) -> Result<Self, String> {
        let messages = history_for_turn_with(turns, turn, user_message)?;
        let mut response = AssistantResponse::new(model, provider);
        let request_info = prepare_response(
            &conversation.id,
            &turn.id,
            &mut response,
            provider,
            model,
            &conversation.system_prompt,
            &messages,
        );
        let provider_request = provider_request(
            provider,
            model,
            &conversation.system_prompt,
            &turn.generation_config,
            messages,
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
        })
    }

    pub fn regenerate(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        turns: &[Turn],
        turn: &Turn,
        previous_response: &AssistantResponse,
        user_message: &dyn Fn(&UserMessage) -> Result<Message, String>,
    ) -> Result<Self, String> {
        let messages = history_for_turn_with(turns, turn, user_message)?;
        let mut response = previous_response.clone();
        let request_info = prepare_response(
            &conversation.id,
            &turn.id,
            &mut response,
            provider,
            model,
            &conversation.system_prompt,
            &messages,
        );
        let mut config = turn.generation_config.clone();
        config
            .reasoning_preset
            .clone_from(&conversation.generation_config.reasoning_preset);
        let provider_request = provider_request(
            provider,
            model,
            &conversation.system_prompt,
            &config,
            messages,
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
        })
    }
}

pub fn history_for_turn(turns: &[Turn], turn: &Turn) -> Vec<Message> {
    history_for_turn_with(turns, turn, &plain_user_message).unwrap_or_default()
}

fn history_for_turn_with(
    turns: &[Turn],
    turn: &Turn,
    user_message: &dyn Fn(&UserMessage) -> Result<Message, String>,
) -> Result<Vec<Message>, String> {
    let mut messages = turn
        .parent_response_id
        .as_deref()
        .map(|response_id| history_through_response(turns, response_id, user_message))
        .transpose()?
        .unwrap_or_default();
    messages.push(user_message(&turn.user)?);
    Ok(messages)
}

pub fn history_for_new_turn(turns: &[Turn]) -> Vec<Message> {
    active_turns(turns)
        .last()
        .and_then(|turn| turn.continuation_response_id.as_deref())
        .and_then(|response_id| {
            history_through_response(turns, response_id, &plain_user_message).ok()
        })
        .unwrap_or_default()
}

fn plain_user_message(user: &UserMessage) -> Result<Message, String> {
    Ok(Message::user(user.content.clone()))
}

fn history_through_response(
    turns: &[Turn],
    response_id: &str,
    user_message: &dyn Fn(&UserMessage) -> Result<Message, String>,
) -> Result<Vec<Message>, String> {
    fn visit(
        turns: &[Turn],
        response_id: &str,
        visited: &mut HashSet<String>,
        messages: &mut Vec<Message>,
        user_message: &dyn Fn(&UserMessage) -> Result<Message, String>,
    ) -> Result<(), String> {
        if !visited.insert(response_id.to_string()) {
            return Ok(());
        }
        let Some((turn, response)) = turns
            .iter()
            .find_map(|turn| turn.response(response_id).map(|response| (turn, response)))
        else {
            return Ok(());
        };
        if let Some(parent_id) = turn.parent_response_id.as_deref() {
            visit(turns, parent_id, visited, messages, user_message)?;
        }
        messages.push(user_message(&turn.user)?);
        if response.transcript.is_empty() {
            if !response.content.is_empty() {
                messages.push(Message::assistant(response.content.clone()));
            }
        } else {
            messages.extend(response.transcript.clone());
        }
        Ok(())
    }

    let mut messages = Vec::new();
    visit(
        turns,
        response_id,
        &mut HashSet::new(),
        &mut messages,
        user_message,
    )?;
    Ok(messages)
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

    let input_text_len = system_prompt.chars().count()
        + messages
            .iter()
            .map(|message| serde_json::to_string(message).map_or(0, |value| value.chars().count()))
            .sum::<usize>();
    request.usage.input_tokens = Some(estimate_tokens(input_text_len));
    request.usage.estimated = true;

    request
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
