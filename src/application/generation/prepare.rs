use std::collections::HashSet;

use super::reducer::estimate_tokens;

use crate::domain::{
    AssistantResponse, ChatMessage, Conversation, GenerationConfig, GenerationRequest, MessageRole,
    MessageStatus, Model, Provider, RequestInfo, Turn, active_turns, now_timestamp,
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
}

impl PreparedGeneration {
    pub fn new(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        turns: &[Turn],
        parent_response_id: Option<String>,
        prompt: String,
    ) -> Self {
        let mut messages = parent_response_id
            .as_deref()
            .map(|response_id| history_through_response(turns, response_id))
            .unwrap_or_default();
        messages.push(ChatMessage {
            role: MessageRole::User,
            content: prompt.clone(),
        });

        let response = AssistantResponse::new(model, provider);
        let mut turn = Turn::new(conversation, parent_response_id, prompt, response);
        let response = &mut turn.responses[0];
        let request_info = prepare_response(
            &conversation.id,
            &turn.id,
            response,
            provider,
            model,
            &conversation.system_prompt.content,
            &messages,
        );
        let response = response.clone();
        let provider_request = provider_request(
            provider,
            model,
            &conversation.system_prompt.content,
            &conversation.generation_config,
            messages,
        );
        Self {
            start: GenerationStart::NewTurn(Box::new(turn)),
            response,
            request_info,
            provider_request,
        }
    }

    pub fn additional(
        conversation_id: &str,
        provider: &Provider,
        model: &Model,
        turns: &[Turn],
        turn: &Turn,
    ) -> Self {
        let messages = history_for_turn(turns, turn);
        let mut response = AssistantResponse::new(model, provider);
        let request_info = prepare_response(
            conversation_id,
            &turn.id,
            &mut response,
            provider,
            model,
            &turn.generation.system_prompt,
            &messages,
        );
        let provider_request = provider_request(
            provider,
            model,
            &turn.generation.system_prompt,
            &turn.generation.config,
            messages,
        );
        Self {
            start: GenerationStart::AddResponse {
                turn_id: turn.id.clone(),
            },
            response,
            request_info,
            provider_request,
        }
    }

    pub fn regenerate(
        conversation_id: &str,
        provider: &Provider,
        model: &Model,
        turns: &[Turn],
        turn: &Turn,
        previous_response: &AssistantResponse,
    ) -> Self {
        let messages = history_for_turn(turns, turn);
        let mut response = previous_response.clone();
        let request_info = prepare_response(
            conversation_id,
            &turn.id,
            &mut response,
            provider,
            model,
            &turn.generation.system_prompt,
            &messages,
        );
        let provider_request = provider_request(
            provider,
            model,
            &turn.generation.system_prompt,
            &turn.generation.config,
            messages,
        );
        Self {
            start: GenerationStart::RetryResponse {
                turn_id: turn.id.clone(),
            },
            response,
            request_info,
            provider_request,
        }
    }
}

pub fn history_for_turn(turns: &[Turn], turn: &Turn) -> Vec<ChatMessage> {
    let mut messages = turn
        .parent_response_id
        .as_deref()
        .map(|response_id| history_through_response(turns, response_id))
        .unwrap_or_default();
    messages.push(ChatMessage {
        role: MessageRole::User,
        content: turn.user.content.clone(),
    });
    messages
}

pub fn history_for_new_turn(turns: &[Turn]) -> Vec<ChatMessage> {
    active_turns(turns)
        .last()
        .and_then(|turn| turn.continuation_response_id.as_deref())
        .map(|response_id| history_through_response(turns, response_id))
        .unwrap_or_default()
}

fn history_through_response(turns: &[Turn], response_id: &str) -> Vec<ChatMessage> {
    fn visit(
        turns: &[Turn],
        response_id: &str,
        visited: &mut HashSet<String>,
        messages: &mut Vec<ChatMessage>,
    ) {
        if !visited.insert(response_id.to_string()) {
            return;
        }
        let Some((turn, response)) = turns
            .iter()
            .find_map(|turn| turn.response(response_id).map(|response| (turn, response)))
        else {
            return;
        };
        if let Some(parent_id) = turn.parent_response_id.as_deref() {
            visit(turns, parent_id, visited, messages);
        }
        messages.push(ChatMessage {
            role: MessageRole::User,
            content: turn.user.content.clone(),
        });
        if !response.content.is_empty() {
            messages.push(ChatMessage {
                role: MessageRole::Assistant,
                content: response.content.clone(),
            });
        }
    }

    let mut messages = Vec::new();
    visit(turns, response_id, &mut HashSet::new(), &mut messages);
    messages
}

fn prepare_response(
    conversation_id: &str,
    turn_id: &str,
    response: &mut AssistantResponse,
    provider: &Provider,
    model: &Model,
    system_prompt: &str,
    messages: &[ChatMessage],
) -> RequestInfo {
    response.status = MessageStatus::Streaming;
    response.content.clear();
    response.thinking.clear();
    response.updated_at = now_timestamp();
    let mut request = RequestInfo::new(conversation_id, turn_id, &response.id);
    request.provider_id = Some(provider.id.clone());
    request.model_id = Some(model.id.clone());
    response.request_id = Some(request.id.clone());

    let input_text_len = system_prompt.chars().count()
        + messages
            .iter()
            .map(|message| message.content.chars().count())
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
    messages: Vec<ChatMessage>,
) -> GenerationRequest {
    let (config, _) = config.filtered_for(&model.capabilities);
    GenerationRequest {
        provider: provider.clone(),
        model: model.clone(),
        system_prompt: system_prompt.to_string(),
        config,
        messages,
    }
}
