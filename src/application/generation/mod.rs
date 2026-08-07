mod active;
mod prepare;
mod reducer;
mod runner;

pub use active::{ActiveGeneration, GenerationManager};
pub use prepare::{GenerationStart, PreparedGeneration, history_for_new_turn, history_for_turn};
pub use reducer::{EventOutcome, apply_event, interrupted_event};
pub use runner::{
    GenerationSnapshot, GenerationUpdate, STORAGE_FLUSH_INTERVAL, UI_FLUSH_INTERVAL, run_generation,
};

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::domain::*;

    fn response(provider: &Provider, model: &Model) -> AssistantResponse {
        AssistantResponse::new(model, provider)
    }

    fn request(response: &AssistantResponse) -> RequestInfo {
        RequestInfo::new("conversation", "turn", &response.id)
    }

    #[test]
    fn preparation_creates_a_turn_and_paired_records() {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        let model = Model::new(&provider.id, "gpt-test", "GPT Test");
        let mut conversation = Conversation::new("Test", Some(&model), "");
        conversation.generation_config.temperature = Some(0.4);
        conversation.generation_config.top_k = Some(20);

        let prepared =
            PreparedGeneration::new(&conversation, &provider, &model, &[], None, "Next".into());
        let GenerationStart::NewTurn(turn) = &prepared.start else {
            panic!("expected a new turn");
        };

        assert_eq!(turn.user.content, "Next");
        assert_eq!(turn.responses, vec![prepared.response.clone()]);
        assert_eq!(
            prepared.response.request_id,
            Some(prepared.request_info.id.clone())
        );
        assert_eq!(prepared.response.status, MessageStatus::Streaming);
        assert_eq!(prepared.request_info.turn_id, turn.id);
        assert_eq!(prepared.provider_request.messages.len(), 1);
        assert_eq!(prepared.provider_request.config.temperature, Some(0.4));
        assert_eq!(prepared.provider_request.config.top_k, None);
    }

    #[test]
    fn additional_responses_use_the_turns_original_parent_path() {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        let model = Model::new(&provider.id, "gpt-test", "GPT Test");
        let other = Model::new(&provider.id, "other", "Other");
        let mut conversation = Conversation::new("Test", Some(&model), "Original prompt");

        let first = PreparedGeneration::new(
            &conversation,
            &provider,
            &model,
            &[],
            None,
            "Question one".into(),
        );
        let GenerationStart::NewTurn(turn_one) = first.start else {
            panic!("expected a new turn");
        };
        let mut turn_one = *turn_one;
        turn_one.responses[0].content = "Chosen answer".into();
        turn_one.responses[0].status = MessageStatus::Completed;

        let second = PreparedGeneration::new(
            &conversation,
            &provider,
            &model,
            std::slice::from_ref(&turn_one),
            turn_one.continuation_response_id.clone(),
            "Question two".into(),
        );
        let GenerationStart::NewTurn(turn_two) = second.start else {
            panic!("expected a new turn");
        };
        let turn_two = *turn_two;
        let turns = vec![turn_one, turn_two.clone()];
        conversation.system_prompt = "Current prompt".into();
        let alternate =
            PreparedGeneration::additional(&conversation, &provider, &other, &turns, &turn_two);

        assert_eq!(alternate.provider_request.system_prompt, "Current prompt");
        assert_eq!(
            alternate
                .provider_request
                .messages
                .iter()
                .map(|message| (message.role, message.content.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (MessageRole::User, "Question one"),
                (MessageRole::Assistant, "Chosen answer"),
                (MessageRole::User, "Question two"),
            ]
        );
    }

    #[test]
    fn edited_user_messages_exclude_the_previous_branch_suffix() {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        let model = Model::new(&provider.id, "gpt-test", "GPT Test");
        let conversation = Conversation::new("Test", Some(&model), "");

        let mut root = Turn::new(
            &conversation,
            None,
            "Question one",
            response(&provider, &model),
        );
        root.responses[0].content = "Answer one".into();
        let root_response_id = root.responses[0].id.clone();
        let mut previous = Turn::new(
            &conversation,
            Some(root_response_id.clone()),
            "Old question",
            response(&provider, &model),
        );
        previous.responses[0].content = "Old answer".into();
        let previous_response_id = previous.responses[0].id.clone();
        let suffix = Turn::new(
            &conversation,
            Some(previous_response_id),
            "Old suffix",
            response(&provider, &model),
        );
        let turns = vec![root, previous, suffix];

        let edited = PreparedGeneration::new(
            &conversation,
            &provider,
            &model,
            &turns,
            Some(root_response_id),
            "Edited question".into(),
        );

        assert_eq!(
            edited
                .provider_request
                .messages
                .iter()
                .map(|message| (message.role, message.content.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (MessageRole::User, "Question one"),
                (MessageRole::Assistant, "Answer one"),
                (MessageRole::User, "Edited question"),
            ]
        );
    }

    #[test]
    fn regeneration_reuses_the_response_id() {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        let model = Model::new(&provider.id, "gpt-test", "GPT Test");
        let mut conversation = Conversation::new("Test", Some(&model), "Original prompt");
        let mut previous = response(&provider, &model);
        previous.content = "Old answer".into();
        previous.thinking = "Old thinking".into();
        let turn = Turn::new(&conversation, None, "Question", previous.clone());
        conversation.system_prompt = "Current prompt".into();

        let prepared = PreparedGeneration::regenerate(
            &conversation,
            &provider,
            &model,
            std::slice::from_ref(&turn),
            &turn,
            &previous,
        );

        assert_eq!(prepared.provider_request.system_prompt, "Current prompt");
        assert_eq!(prepared.response.id, previous.id);
        assert_eq!(prepared.response.status, MessageStatus::Streaming);
        assert!(prepared.response.content.is_empty());
        assert!(prepared.response.thinking.is_empty());
        assert_ne!(prepared.response.request_id, previous.request_id);
    }

    #[test]
    fn events_preserve_partial_text_when_stopped() {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        let model = Model::new(&provider.id, "model", "Model");
        let mut response = response(&provider, &model);
        response.status = MessageStatus::Streaming;
        let mut request = request(&response);

        assert!(
            !apply_event(
                GenerationEvent::TextDelta("partial".into()),
                &mut response,
                &mut request,
                Duration::from_millis(25),
            )
            .terminal
        );
        assert!(
            apply_event(
                GenerationEvent::Failed(GenerationError::cancelled()),
                &mut response,
                &mut request,
                Duration::from_millis(80),
            )
            .terminal
        );

        assert_eq!(response.content, "partial");
        assert_eq!(response.status, MessageStatus::Stopped);
        assert_eq!(request.status, RequestStatus::Stopped);
        assert_eq!(request.ttft_ms, Some(25));
        assert_eq!(request.duration_ms, Some(80));
    }

    #[test]
    fn reasoning_duration_stops_when_answer_text_starts() {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        let model = Model::new(&provider.id, "model", "Model");
        let mut response = response(&provider, &model);
        let mut request = request(&response);

        let thinking = apply_event(
            GenerationEvent::ThinkingDelta("Working".into()),
            &mut response,
            &mut request,
            Duration::from_millis(400),
        );
        let first_text = apply_event(
            GenerationEvent::TextDelta("Done".into()),
            &mut response,
            &mut request,
            Duration::from_millis(1_250),
        );
        let second_text = apply_event(
            GenerationEvent::TextDelta(".".into()),
            &mut response,
            &mut request,
            Duration::from_millis(1_500),
        );

        assert!(!thinking.thinking_finished);
        assert!(first_text.thinking_finished);
        assert!(!second_text.thinking_finished);
        assert_eq!(request.thinking_duration_ms, Some(1_250));
    }

    #[test]
    fn reasoning_only_generation_signals_completion() {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        let model = Model::new(&provider.id, "model", "Model");
        let mut response = response(&provider, &model);
        let mut request = request(&response);
        apply_event(
            GenerationEvent::ThinkingDelta("Working".into()),
            &mut response,
            &mut request,
            Duration::from_millis(400),
        );

        let completed = apply_event(
            GenerationEvent::Completed,
            &mut response,
            &mut request,
            Duration::from_millis(800),
        );

        assert!(completed.terminal);
        assert!(completed.thinking_finished);
        assert_eq!(request.thinking_duration_ms, Some(800));
    }

    #[test]
    fn usage_and_failure_are_recorded() {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        let model = Model::new(&provider.id, "model", "Model");
        let mut response = response(&provider, &model);
        let mut request = request(&response);
        apply_event(
            GenerationEvent::UsageUpdated(TokenUsage {
                input_tokens: Some(4),
                output_tokens: Some(9),
                estimated: false,
            }),
            &mut response,
            &mut request,
            Duration::ZERO,
        );
        apply_event(
            GenerationEvent::Failed(GenerationError::new(
                GenerationErrorKind::RateLimited,
                "Slow down",
            )),
            &mut response,
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
            "response".into(),
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
