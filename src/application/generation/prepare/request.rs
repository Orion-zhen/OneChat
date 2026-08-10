use super::*;

pub(super) fn prepare_response(
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

pub(super) fn update_input_token_estimate(
    request: &mut RequestInfo,
    provider_request: &GenerationRequest,
) {
    request.usage.input_tokens = Some(estimate_input_tokens(
        &provider_request.system_prompt,
        &provider_request.messages,
    ));
    request.usage.estimated = true;
}

pub(super) fn provider_request(
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
