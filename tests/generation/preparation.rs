use super::*;

#[test]
fn generation_preparation_uses_the_selected_history_and_model_capabilities() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let mut model = Model::new(&provider.id, "test-model", "Test Model");
    model.capabilities.top_k = false;
    let mut conversation = Conversation::new("Chat", Some(&model), "  Be concise.  ");
    conversation.assistant_opening = "Welcome, {{owner}}.".into();
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
    assert_eq!(messages.len(), 4);
    assert!(messages[0].contains("Welcome, {{owner}}."));
    assert!(messages[1].contains("first question"));
    assert!(messages[2].contains("first answer"));
    assert!(messages[3].contains("follow-up"));
}

#[test]
fn continuation_ends_at_the_existing_assistant_message_without_an_instruction() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let conversation = Conversation::new("Chat", Some(&model), "System");
    let root = completed_turn(
        &conversation,
        None,
        "root question",
        "root answer",
        &model,
        &provider,
    );
    let turn = completed_turn(
        &conversation,
        Some(root.responses[0].id.clone()),
        "current question",
        "existing answer",
        &model,
        &provider,
    );
    let response = turn.responses[0].clone();
    let turns = [root, turn.clone()];

    let prepared = PreparedGeneration::continuation(
        &conversation,
        &provider,
        &model,
        &turns,
        &turn,
        &response,
        ContextPolicy::new(HistoryLimit::Unlimited, &|user| {
            Ok(Message::user(user.content.clone()))
        }),
    )
    .unwrap();

    let GenerationStart::ContinueResponse { turn_id } = &prepared.start else {
        panic!("expected a continued response");
    };
    assert_eq!(turn_id, &turn.id);
    assert_eq!(prepared.request_info.kind, RequestKind::Continue);
    assert_eq!(prepared.response.id, response.id);
    assert_eq!(prepared.response.content, "existing answer");
    assert_eq!(
        prepared
            .continuation_baseline
            .as_ref()
            .map(|response| response.content.as_str()),
        Some("existing answer")
    );

    let messages = serialized_messages(&prepared.provider_request.messages);
    assert_eq!(messages.len(), 4);
    assert!(messages[0].contains("root question"));
    assert!(messages[1].contains("root answer"));
    assert!(messages[2].contains("current question"));
    assert!(messages[3].contains("existing answer"));
    assert!(matches!(
        prepared.provider_request.messages.last(),
        Some(Message::Assistant { .. })
    ));
    assert!(!messages.join("\n").contains("Continue"));
}

#[tokio::test]
async fn assistant_opening_variables_are_resolved_and_snapshotted() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let mut conversation = Conversation::new("Chat", Some(&model), "For {{owner}}");
    conversation.assistant_opening = "Welcome, {{owner}} on {{onechat.os}}.".into();
    let mut prepared = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &[],
        None,
        UserMessage::new("Hello", Vec::new()),
        ContextPolicy::new(HistoryLimit::Unlimited, &|user| {
            Ok(Message::user(user.content.clone()))
        }),
    )
    .unwrap();
    prepared.configure_prompt(
        BTreeMap::from([(
            "owner".into(),
            PromptVariableSource::Text {
                value: "Orion".into(),
            },
        )]),
        PromptContext::default(),
    );

    prepared
        .render_prompt_setup(CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(prepared.provider_request.system_prompt, "For Orion");
    let messages = serialized_messages(&prepared.provider_request.messages);
    assert!(messages[0].contains("Welcome, Orion on"));
    assert!(messages[1].contains("Hello"));
    let opening = prepared.request_info.assistant_opening.unwrap();
    assert_eq!(opening.template, conversation.assistant_opening);
    assert!(opening.resolved.contains("Welcome, Orion on"));
    assert_eq!(opening.variables.len(), 2);
}

#[test]
fn existing_turn_preparation_preserves_start_config_context_request_and_tools() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let mut conversation = Conversation::new("Chat", Some(&model), "System");
    conversation.generation_config.temperature = Some(0.2);
    conversation.generation_config.reasoning_preset = Some("original".into());
    conversation.tool_selection =
        ToolSelection::Only(BTreeSet::from([ToolRef::new("server", "tool")]));

    let root = completed_turn(
        &conversation,
        None,
        "root question",
        "root answer",
        &model,
        &provider,
    );
    let turn = completed_turn(
        &conversation,
        Some(root.responses[0].id.clone()),
        "question",
        "old target answer",
        &model,
        &provider,
    );
    let previous_response = turn.responses[0].clone();
    let turns = [root, turn.clone()];
    conversation.generation_config.temperature = Some(0.9);
    conversation.generation_config.reasoning_preset = Some("current".into());
    let loader = |user: &UserMessage| Ok(Message::user(user.content.clone()));

    let additional = PreparedGeneration::additional(
        &conversation,
        &provider,
        &model,
        &turns,
        &turn,
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    let regenerated = PreparedGeneration::regenerate(
        &conversation,
        &provider,
        &model,
        &turns,
        &turn,
        &previous_response,
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();

    let GenerationStart::AddResponse { turn_id } = &additional.start else {
        panic!("expected an additional response");
    };
    assert_eq!(turn_id, &turn.id);
    let GenerationStart::RetryResponse { turn_id } = &regenerated.start else {
        panic!("expected a retried response");
    };
    assert_eq!(turn_id, &turn.id);
    assert_ne!(additional.response.id, previous_response.id);
    assert_eq!(regenerated.response.id, previous_response.id);

    let expected_context = Some(RequestContextInfo {
        history_limit: HistoryLimit::Unlimited,
        available_history_turns: 1,
        included_history_turns: 1,
        limited_by_context_window: false,
    });
    for prepared in [&additional, &regenerated] {
        assert_eq!(prepared.request_info.conversation_id, conversation.id);
        assert_eq!(prepared.request_info.turn_id, turn.id);
        assert_eq!(prepared.request_info.response_id, prepared.response.id);
        assert_eq!(
            prepared.request_info.provider_id.as_ref(),
            Some(&provider.id)
        );
        assert_eq!(prepared.request_info.model_id.as_ref(), Some(&model.id));
        assert_eq!(prepared.request_info.status, RequestStatus::Sending);
        assert_eq!(prepared.request_info.context, expected_context);
        assert!(prepared.request_info.usage.input_tokens.is_some());
        assert!(prepared.request_info.usage.estimated);
        assert_eq!(
            prepared.response.request_id.as_ref(),
            Some(&prepared.request_info.id)
        );
        assert_eq!(prepared.response.status, MessageStatus::Streaming);
        assert_eq!(prepared.tool_selection, conversation.tool_selection);
        assert_eq!(prepared.provider_request.system_prompt, "System");
        assert_eq!(prepared.provider_request.provider.id, provider.id);
        assert_eq!(prepared.provider_request.model.id, model.id);
        let messages = serialized_messages(&prepared.provider_request.messages).join("\n");
        assert!(messages.contains("root question"));
        assert!(messages.contains("root answer"));
        assert!(messages.contains("question"));
        assert!(!messages.contains("old target answer"));
    }
    assert_eq!(additional.provider_request.config.temperature, Some(0.2));
    assert_eq!(
        additional
            .provider_request
            .config
            .reasoning_preset
            .as_deref(),
        Some("original")
    );
    assert_eq!(regenerated.provider_request.config.temperature, Some(0.2));
    assert_eq!(
        regenerated
            .provider_request
            .config
            .reasoning_preset
            .as_deref(),
        Some("current")
    );
}
