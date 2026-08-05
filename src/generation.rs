use std::{collections::HashMap, time::Duration};

use tokio_util::sync::CancellationToken;

use crate::{
    model::{
        Conversation, Message, MessageRole, MessageStatus, Model, Provider, RequestError,
        RequestInfo, RequestStatus, now_timestamp,
    },
    providers::{AppError, AppErrorKind, ChatMessage, GenerationEvent, GenerationRequest},
};

pub const UI_FLUSH_INTERVAL: Duration = Duration::from_millis(40);
pub const DATABASE_FLUSH_INTERVAL: Duration = Duration::from_millis(320);

pub struct ActiveGeneration {
    pub request_id: String,
    pub assistant_message_id: String,
    pub cancellation: CancellationToken,
}

#[derive(Default)]
pub struct GenerationManager {
    active: HashMap<String, ActiveGeneration>,
}

impl GenerationManager {
    pub fn is_active(&self, conversation_id: &str) -> bool {
        self.active.contains_key(conversation_id)
    }

    pub fn start(
        &mut self,
        conversation_id: String,
        request_id: String,
        assistant_message_id: String,
        cancellation: CancellationToken,
    ) -> bool {
        if self.active.contains_key(&conversation_id) {
            return false;
        }
        self.active.insert(
            conversation_id,
            ActiveGeneration {
                request_id,
                assistant_message_id,
                cancellation,
            },
        );
        true
    }

    pub fn stop(&self, conversation_id: &str) -> bool {
        let Some(active) = self.active.get(conversation_id) else {
            return false;
        };
        active.cancellation.cancel();
        true
    }

    pub fn finish(&mut self, conversation_id: &str, request_id: &str) {
        if self
            .active
            .get(conversation_id)
            .is_some_and(|active| active.request_id == request_id)
        {
            self.active.remove(conversation_id);
        }
    }

    pub fn active_request(&self, conversation_id: &str) -> Option<&ActiveGeneration> {
        self.active.get(conversation_id)
    }
}

#[derive(Clone)]
pub struct PreparedGeneration {
    pub user: Message,
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
        let mut assistant = Message::new(&conversation.id, MessageRole::Assistant, "");
        assistant.status = MessageStatus::Streaming;
        let mut request_info = RequestInfo::new(&conversation.id, &assistant.id);
        request_info.provider_id = Some(provider.id.clone());
        request_info.model_id = Some(model.id.clone());
        assistant.request_id = Some(request_info.id.clone());

        let mut messages = context
            .iter()
            .filter(|message| !message.content.is_empty())
            .map(|message| ChatMessage {
                role: message.role,
                content: message.content.clone(),
            })
            .collect::<Vec<_>>();
        messages.push(ChatMessage {
            role: MessageRole::User,
            content: user.content.clone(),
        });
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

pub fn apply_event(
    event: GenerationEvent,
    assistant: &mut Message,
    request: &mut RequestInfo,
    elapsed: Duration,
) -> bool {
    match event {
        GenerationEvent::Started => {
            request.status = RequestStatus::Streaming;
            false
        }
        GenerationEvent::TextDelta(delta) => {
            mark_first_token(request, elapsed);
            assistant.content.push_str(&delta);
            assistant.updated_at = now_timestamp();
            false
        }
        GenerationEvent::ThinkingDelta(delta) => {
            mark_first_token(request, elapsed);
            assistant.thinking.push_str(&delta);
            assistant.updated_at = now_timestamp();
            false
        }
        GenerationEvent::UsageUpdated(usage) => {
            request.usage = usage;
            false
        }
        GenerationEvent::Completed => {
            estimate_output_usage(assistant, request);
            assistant.status = MessageStatus::Completed;
            finish_request(request, RequestStatus::Completed, elapsed);
            true
        }
        GenerationEvent::Failed(error) => {
            estimate_output_usage(assistant, request);
            let cancelled = error.kind == AppErrorKind::UserCancelled;
            assistant.status = if cancelled {
                MessageStatus::Stopped
            } else {
                MessageStatus::Failed
            };
            request.error = (!cancelled).then(|| request_error(&error));
            finish_request(
                request,
                if cancelled {
                    RequestStatus::Stopped
                } else {
                    RequestStatus::Failed
                },
                elapsed,
            );
            true
        }
    }
}

pub fn interrupted_event() -> GenerationEvent {
    GenerationEvent::Failed(AppError::new(
        AppErrorKind::StreamInterrupted,
        "Provider stream closed unexpectedly",
    ))
}

fn mark_first_token(request: &mut RequestInfo, elapsed: Duration) {
    if request.ttft_ms.is_none() {
        request.first_token_at = Some(now_timestamp());
        request.ttft_ms = Some(elapsed.as_millis() as u64);
    }
    request.status = RequestStatus::Streaming;
}

fn finish_request(request: &mut RequestInfo, status: RequestStatus, elapsed: Duration) {
    request.status = status;
    request.finished_at = Some(now_timestamp());
    request.duration_ms = Some(elapsed.as_millis() as u64);
}

fn estimate_output_usage(assistant: &Message, request: &mut RequestInfo) {
    if request.usage.output_tokens.is_none() {
        request.usage.output_tokens = Some(estimate_tokens(
            assistant.content.chars().count() + assistant.thinking.chars().count(),
        ));
        request.usage.estimated = true;
    }
}

fn estimate_tokens(characters: usize) -> u64 {
    characters.div_ceil(4) as u64
}

fn request_error(error: &AppError) -> RequestError {
    RequestError {
        kind: error.kind.as_str().into(),
        message: error.message.clone(),
        detail: error.detail.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProviderKind, TokenUsage};

    #[test]
    fn preparation_includes_context_and_paired_database_rows() {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        let model = Model::new(&provider.id, "gpt-test", "GPT Test");
        let mut conversation = Conversation::new("Test", Some(&model), "");
        conversation.generation_config.temperature = Some(0.4);
        conversation.generation_config.top_k = Some(20);
        let context = vec![Message::new(
            &conversation.id,
            MessageRole::Assistant,
            "Earlier",
        )];

        let prepared =
            PreparedGeneration::new(&conversation, &provider, &model, &context, "Next".into());

        assert_eq!(prepared.provider_request.messages.len(), 2);
        assert_eq!(
            prepared.assistant.request_id,
            Some(prepared.request_info.id.clone())
        );
        assert_eq!(prepared.assistant.status, MessageStatus::Streaming);
        assert_eq!(prepared.request_info.status, RequestStatus::Sending);
        assert!(prepared.request_info.usage.estimated);
        assert!(prepared.request_info.usage.input_tokens.is_some());
        assert_eq!(prepared.provider_request.config.temperature, Some(0.4));
        assert_eq!(prepared.provider_request.config.top_k, None);
        assert_eq!(conversation.generation_config.top_k, Some(20));
    }

    #[test]
    fn events_preserve_partial_text_when_stopped() {
        let mut message = Message::new("conversation", MessageRole::Assistant, "");
        message.status = MessageStatus::Streaming;
        let mut request = RequestInfo::new("conversation", &message.id);

        assert!(!apply_event(
            GenerationEvent::TextDelta("partial".into()),
            &mut message,
            &mut request,
            Duration::from_millis(25),
        ));
        assert!(apply_event(
            GenerationEvent::Failed(AppError::cancelled()),
            &mut message,
            &mut request,
            Duration::from_millis(80),
        ));

        assert_eq!(message.content, "partial");
        assert_eq!(message.status, MessageStatus::Stopped);
        assert_eq!(request.status, RequestStatus::Stopped);
        assert_eq!(request.ttft_ms, Some(25));
        assert_eq!(request.duration_ms, Some(80));
    }

    #[test]
    fn usage_and_failure_are_recorded() {
        let mut message = Message::new("conversation", MessageRole::Assistant, "");
        let mut request = RequestInfo::new("conversation", &message.id);
        apply_event(
            GenerationEvent::UsageUpdated(TokenUsage {
                input_tokens: Some(4),
                output_tokens: Some(9),
                estimated: false,
            }),
            &mut message,
            &mut request,
            Duration::ZERO,
        );
        apply_event(
            GenerationEvent::Failed(AppError::new(AppErrorKind::RateLimited, "Slow down")),
            &mut message,
            &mut request,
            Duration::from_millis(100),
        );

        assert_eq!(request.usage.output_tokens, Some(9));
        assert_eq!(request.status, RequestStatus::Failed);
        assert_eq!(request.error.unwrap().kind, "rate_limited");
    }

    #[test]
    fn flush_intervals_stay_within_the_ui_and_database_budgets() {
        assert!((30..=50).contains(&UI_FLUSH_INTERVAL.as_millis()));
        assert!((250..=500).contains(&DATABASE_FLUSH_INTERVAL.as_millis()));
    }

    #[test]
    fn manager_allows_one_generation_per_conversation() {
        let mut manager = GenerationManager::default();
        assert!(manager.start(
            "conversation".into(),
            "request".into(),
            "message".into(),
            CancellationToken::new(),
        ));
        assert!(!manager.start(
            "conversation".into(),
            "other".into(),
            "other".into(),
            CancellationToken::new(),
        ));
        assert!(manager.stop("conversation"));
        manager.finish("conversation", "request");
        assert!(!manager.is_active("conversation"));
    }
}
