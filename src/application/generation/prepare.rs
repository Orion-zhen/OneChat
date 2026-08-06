use super::reducer::estimate_tokens;

use crate::domain::{
    ChatMessage, Conversation, GenerationRequest, Message, MessageRole, MessageStatus, Model,
    Provider, RequestInfo, now_timestamp,
};

#[derive(Clone)]
pub struct PreparedGeneration {
    pub user: Option<Message>,
    pub assistant: Message,
    pub request_info: RequestInfo,
    pub provider_request: GenerationRequest,
}

impl PreparedGeneration {
    pub fn new(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        context: &[Message],
        prompt: String,
    ) -> Self {
        let user = Message::new(&conversation.id, MessageRole::User, prompt);
        let mut messages = chat_messages(context);
        messages.push(ChatMessage {
            role: MessageRole::User,
            content: user.content.clone(),
        });
        Self::prepare(conversation, provider, model, messages, Some(user), None)
    }

    pub fn regenerate(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        context: &[Message],
        previous_assistant: &Message,
    ) -> Self {
        Self::prepare(
            conversation,
            provider,
            model,
            chat_messages(context),
            None,
            Some(previous_assistant.clone()),
        )
    }

    fn prepare(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        messages: Vec<ChatMessage>,
        user: Option<Message>,
        previous_assistant: Option<Message>,
    ) -> Self {
        let mut assistant = previous_assistant
            .unwrap_or_else(|| Message::new(&conversation.id, MessageRole::Assistant, ""));
        assistant.status = MessageStatus::Streaming;
        assistant.content.clear();
        assistant.thinking.clear();
        assistant.updated_at = now_timestamp();
        let mut request_info = RequestInfo::new(&conversation.id, &assistant.id);
        request_info.provider_id = Some(provider.id.clone());
        request_info.model_id = Some(model.id.clone());
        assistant.request_id = Some(request_info.id.clone());

        let input_text_len = conversation.system_prompt.content.chars().count()
            + messages
                .iter()
                .map(|message| message.content.chars().count())
                .sum::<usize>();
        request_info.usage.input_tokens = Some(estimate_tokens(input_text_len));
        request_info.usage.estimated = true;

        let (config, _) = conversation
            .generation_config
            .filtered_for(&model.capabilities);
        Self {
            user,
            assistant,
            request_info,
            provider_request: GenerationRequest {
                provider: provider.clone(),
                model: model.clone(),
                system_prompt: conversation.system_prompt.content.clone(),
                config,
                messages,
            },
        }
    }
}

fn chat_messages(context: &[Message]) -> Vec<ChatMessage> {
    context
        .iter()
        .filter(|message| !message.content.is_empty())
        .map(|message| ChatMessage {
            role: message.role,
            content: message.content.clone(),
        })
        .collect()
}
