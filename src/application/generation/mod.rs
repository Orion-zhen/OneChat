mod active;
mod prepare;
mod reducer;
mod runner;

pub use active::{ActiveGeneration, GenerationManager};
pub use prepare::PreparedGeneration;
pub use reducer::{apply_event, interrupted_event};
pub use runner::{
    GenerationSnapshot, GenerationUpdate, STORAGE_FLUSH_INTERVAL, UI_FLUSH_INTERVAL, run_generation,
};

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::domain::*;

    #[test]
    fn preparation_includes_context_and_paired_storage_records() {
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
        assert_eq!(prepared.user.as_ref().unwrap().content, "Next");
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
    fn regeneration_reuses_the_assistant_message_without_adding_a_user_message() {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        let model = Model::new(&provider.id, "gpt-test", "GPT Test");
        let conversation = Conversation::new("Test", Some(&model), "");
        let user = Message::new(&conversation.id, MessageRole::User, "Question");
        let mut assistant = Message::new(&conversation.id, MessageRole::Assistant, "Old answer");
        assistant.thinking = "Old thinking".into();
        assistant.request_id = Some("old-request".into());

        let prepared = PreparedGeneration::regenerate(
            &conversation,
            &provider,
            &model,
            std::slice::from_ref(&user),
            &assistant,
        );

        assert!(prepared.user.is_none());
        assert_eq!(prepared.assistant.id, assistant.id);
        assert_eq!(prepared.assistant.status, MessageStatus::Streaming);
        assert!(prepared.assistant.content.is_empty());
        assert!(prepared.assistant.thinking.is_empty());
        assert_ne!(prepared.assistant.request_id, assistant.request_id);
        assert_eq!(prepared.provider_request.messages.len(), 1);
        assert_eq!(prepared.provider_request.messages[0].content, "Question");
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
            GenerationEvent::Failed(GenerationError::cancelled()),
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
    fn reasoning_duration_stops_when_answer_text_starts() {
        let mut message = Message::new("conversation", MessageRole::Assistant, "");
        message.status = MessageStatus::Streaming;
        let mut request = RequestInfo::new("conversation", &message.id);

        apply_event(
            GenerationEvent::ThinkingDelta("Working".into()),
            &mut message,
            &mut request,
            Duration::from_millis(400),
        );
        apply_event(
            GenerationEvent::TextDelta("Done".into()),
            &mut message,
            &mut request,
            Duration::from_millis(1_250),
        );
        apply_event(
            GenerationEvent::TextDelta(".".into()),
            &mut message,
            &mut request,
            Duration::from_millis(1_500),
        );

        assert_eq!(request.thinking_duration_ms, Some(1_250));
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
            GenerationEvent::Failed(GenerationError::new(
                GenerationErrorKind::RateLimited,
                "Slow down",
            )),
            &mut message,
            &mut request,
            Duration::from_millis(100),
        );

        assert_eq!(request.usage.output_tokens, Some(9));
        assert_eq!(request.status, RequestStatus::Failed);
        assert_eq!(request.error.unwrap().kind, "rate_limited");
    }

    #[test]
    fn flush_intervals_stay_within_the_ui_and_storage_budgets() {
        assert!((30..=50).contains(&UI_FLUSH_INTERVAL.as_millis()));
        assert!((250..=500).contains(&STORAGE_FLUSH_INTERVAL.as_millis()));
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
