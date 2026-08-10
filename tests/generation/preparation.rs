use super::*;

#[test]
fn generation_preparation_uses_the_selected_history_and_model_capabilities() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let mut model = Model::new(&provider.id, "test-model", "Test Model");
    model.capabilities.top_k = false;
    let mut conversation = Conversation::new("Chat", Some(&model), "  Be concise.  ");
    conversation.generation_config = GenerationConfig {
        temperature: Some(0.4),
        top_k: Some(20),
        ..GenerationConfig::default()
    };

    let root = completed_turn(
        &conversation,
        None,
        "first question",
        "first answer",
        &model,
        &provider,
    );
    let root_response_id = root.responses[0].id.clone();
    let prepared = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &[root],
        Some(root_response_id.clone()),
        UserMessage::new("follow-up", Vec::new()),
        ContextPolicy::new(HistoryLimit::Unlimited, &|user| {
            Ok(Message::user(user.content.clone()))
        }),
    )
    .unwrap();

    let GenerationStart::NewTurn(turn) = &prepared.start else {
        panic!("expected a new turn");
    };
    assert_eq!(
        turn.parent_response_id.as_deref(),
        Some(root_response_id.as_str())
    );
    assert_eq!(prepared.response.status, MessageStatus::Streaming);
    assert_eq!(
        prepared.response.request_id.as_deref(),
        Some(prepared.request_info.id.as_str())
    );
    assert_eq!(prepared.request_info.status, RequestStatus::Sending);
    assert!(prepared.request_info.usage.input_tokens.is_some());
    assert!(prepared.request_info.usage.estimated);
    assert_eq!(prepared.provider_request.system_prompt, "Be concise.");
    assert_eq!(prepared.provider_request.config.temperature, Some(0.4));
    assert_eq!(prepared.provider_request.config.top_k, None);

    let messages = serialized_messages(&prepared.provider_request.messages);
    assert_eq!(messages.len(), 3);
    assert!(messages[0].contains("first question"));
    assert!(messages[1].contains("first answer"));
    assert!(messages[2].contains("follow-up"));
}

#[test]
fn regeneration_uses_the_current_reasoning_preset() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let mut conversation = Conversation::new("Chat", Some(&model), "");
    conversation.generation_config.temperature = Some(0.2);
    conversation.generation_config.reasoning_preset = Some("original".into());

    let turn = completed_turn(&conversation, None, "question", "answer", &model, &provider);
    let previous_response = turn.responses[0].clone();
    conversation.generation_config.temperature = Some(0.9);
    conversation.generation_config.reasoning_preset = Some("current".into());

    let prepared = PreparedGeneration::regenerate(
        &conversation,
        &provider,
        &model,
        std::slice::from_ref(&turn),
        &turn,
        &previous_response,
        ContextPolicy::new(HistoryLimit::Unlimited, &|user| {
            Ok(Message::user(user.content.clone()))
        }),
    )
    .unwrap();

    assert_eq!(prepared.provider_request.config.temperature, Some(0.2));
    assert_eq!(
        prepared.provider_request.config.reasoning_preset.as_deref(),
        Some("current")
    );
}
