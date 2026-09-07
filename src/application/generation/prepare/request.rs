use super::context::PreparedContext;
use super::*;

#[derive(Clone)]
pub struct PreparedRequest {
    pub provider: Provider,
    pub model: Model,
    pub system_prompt: String,
    pub config: GenerationConfig,
    pub(super) context: PreparedContext,
}

impl PreparedRequest {
    pub(super) fn new(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        config: &GenerationConfig,
        mut context: PreparedContext,
    ) -> Self {
        if !conversation.assistant_opening.is_empty() {
            context.set_opening(conversation.assistant_opening.clone());
        }
        let (config, _) = config.filtered_for(&model.capabilities);
        Self {
            provider: provider.clone(),
            model: model.clone(),
            system_prompt: conversation.system_prompt.clone(),
            config,
            context,
        }
    }

    pub fn into_request(self) -> GenerationRequest {
        let audio_duration_ms = self.context.estimate().audio_duration_ms();
        GenerationRequest {
            provider: self.provider,
            model: self.model,
            system_prompt: self.system_prompt,
            config: self.config,
            messages: self.context.into_messages(),
            audio_duration_ms,
            tools: Vec::new(),
        }
    }

    pub(super) fn update_request_info(&self, info: &mut RequestInfo) {
        info.usage.input_tokens = Some(self.context.estimate().tokens(&self.system_prompt));
        info.usage.estimated = true;
        info.context = Some(self.context.request_context);
    }
}

pub(super) fn prepare_response(
    conversation_id: &str,
    turn_id: &str,
    response: &mut AssistantResponse,
    kind: RequestKind,
    input: &PreparedRequest,
) -> RequestInfo {
    if kind == RequestKind::Continue {
        response.prepare_continuation();
    } else {
        response.blocks.clear();
        response.transcript.clear();
        response.tool_executions.clear();
    }
    response.status = MessageStatus::Streaming;
    response.updated_at = now_timestamp();
    let mut request = RequestInfo::new(conversation_id, turn_id, &response.id);
    request.kind = kind;
    request.provider_id = Some(input.provider.id.clone());
    request.model_id = Some(input.model.id.clone());
    response.request_id = Some(request.id.clone());
    input.update_request_info(&mut request);
    request
}
