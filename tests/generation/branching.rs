use super::*;

#[test]
fn additional_and_regenerated_responses_only_use_target_ancestors_and_user_message() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let conversation = Conversation::new("Chat", Some(&model), "");
    let root = completed_turn(
        &conversation,
        None,
        "root question",
        "root answer",
        &model,
        &provider,
    );
    let target = completed_turn(
        &conversation,
        Some(root.responses[0].id.clone()),
        "target question",
        "old target answer",
        &model,
        &provider,
    );
    let descendant = completed_turn(
        &conversation,
        Some(target.responses[0].id.clone()),
        "descendant question",
        "descendant answer",
        &model,
        &provider,
    );
    let turns = [root, target.clone(), descendant];
    let loader = |user: &UserMessage| Ok(Message::user(user.content.clone()));

    let preview = history_for_turn(&turns, &target, HistoryLimit::Last(0));
    let preview = serialized_messages(&preview).join("\n");
    assert!(preview.contains("target question"));
    assert!(!preview.contains("root question"));

    let additional = PreparedGeneration::additional(
        &conversation,
        &provider,
        &model,
        &turns,
        &target,
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    let regenerated = PreparedGeneration::regenerate(
        &conversation,
        &provider,
        &model,
        &turns,
        &target,
        &target.responses[0],
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    for messages in [
        &additional.provider_request.messages,
        &regenerated.provider_request.messages,
    ] {
        let messages = serialized_messages(messages).join("\n");
        assert!(messages.contains("root question"));
        assert!(messages.contains("root answer"));
        assert!(messages.contains("target question"));
        assert!(!messages.contains("old target answer"));
        assert!(!messages.contains("descendant"));
    }
}

#[test]
fn visual_attachments_only_require_vision_when_their_turn_is_retained() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "text-model", "Text Model");
    let conversation = Conversation::new("Chat", Some(&model), "");
    let mut root = completed_turn(
        &conversation,
        None,
        "visual root",
        "root answer",
        &model,
        &provider,
    );
    root.user.attachments.push(image_attachment());
    let mut recent = completed_turn(
        &conversation,
        Some(root.responses[0].id.clone()),
        "recent text",
        "recent answer",
        &model,
        &provider,
    );
    let turns = [root, recent.clone()];
    let loader = |user: &UserMessage| Ok(Message::user(user.content.clone()));

    let mut excluded = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(recent.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Last(1), &loader),
    )
    .unwrap();
    let recent_only_tokens = excluded.request_info.usage.input_tokens.unwrap();
    excluded.finalize_context().unwrap();

    let mut window_trimmed = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(recent.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    window_trimmed.provider_request.model.context_window_tokens = Some(recent_only_tokens as u32);
    window_trimmed.finalize_context().unwrap();
    assert_eq!(
        window_trimmed
            .request_info
            .context
            .unwrap()
            .included_history_turns,
        1
    );

    let mut retained = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(recent.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    assert_eq!(
        retained.finalize_context().unwrap_err().kind,
        GenerationErrorKind::UnsupportedParameter
    );

    recent.user.attachments.push(image_attachment());
    let turns = [turns[0].clone(), recent.clone()];
    let mut retained = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(recent.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Last(1), &loader),
    )
    .unwrap();
    assert!(retained.finalize_context().is_err());

    let mut current = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(recent.responses[0].id.clone()),
        UserMessage::new("current", vec![image_attachment()]),
        ContextPolicy::new(HistoryLimit::Last(0), &loader),
    )
    .unwrap();
    assert!(current.finalize_context().is_err());
}

#[test]
fn audio_only_requires_support_when_its_turn_is_retained() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "text-model", "Text Model");
    let conversation = Conversation::new("Chat", Some(&model), "");
    let mut root = completed_turn(
        &conversation,
        None,
        "audio root",
        "root answer",
        &model,
        &provider,
    );
    root.user.attachments.push(audio_attachment());
    let recent = completed_turn(
        &conversation,
        Some(root.responses[0].id.clone()),
        "recent text",
        "recent answer",
        &model,
        &provider,
    );
    let turns = [root, recent.clone()];
    let loader = |user: &UserMessage| Ok(Message::user(user.content.clone()));

    let mut excluded = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(recent.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Last(1), &loader),
    )
    .unwrap();
    let recent_only_tokens = excluded.request_info.usage.input_tokens.unwrap();
    excluded.finalize_context().unwrap();

    let mut window_trimmed = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(recent.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    window_trimmed.provider_request.model.context_window_tokens = Some(recent_only_tokens as u32);
    window_trimmed.finalize_context().unwrap();
    assert_eq!(
        window_trimmed
            .request_info
            .context
            .unwrap()
            .included_history_turns,
        1
    );

    let mut retained = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(recent.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    let error = retained.finalize_context().unwrap_err();
    assert_eq!(error.kind, GenerationErrorKind::UnsupportedParameter);
    assert!(error.message.contains("audio"));
    assert!(error.message.contains("retained conversation context"));

    let current_user = UserMessage::new("current", vec![audio_attachment()]);
    let mut current = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(recent.responses[0].id.clone()),
        current_user,
        ContextPolicy::new(HistoryLimit::Last(0), &loader),
    )
    .unwrap();
    let error = current.finalize_context().unwrap_err();
    assert!(error.message.contains("audio"));
    assert!(error.message.contains("current message"));
}

#[test]
fn additional_and_regenerated_audio_messages_apply_the_same_capability_check() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "text-model", "Text Model");
    let conversation = Conversation::new("Chat", Some(&model), "");
    let mut turn = completed_turn(
        &conversation,
        None,
        "audio question",
        "answer",
        &model,
        &provider,
    );
    turn.user.attachments.push(audio_attachment());
    let turns = [turn.clone()];
    let loader = |user: &UserMessage| Ok(Message::user(user.content.clone()));

    let mut additional = PreparedGeneration::additional(
        &conversation,
        &provider,
        &model,
        &turns,
        &turn,
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    assert!(
        additional
            .finalize_context()
            .unwrap_err()
            .message
            .contains("audio")
    );

    let mut regenerated = PreparedGeneration::regenerate(
        &conversation,
        &provider,
        &model,
        &turns,
        &turn,
        &turn.responses[0],
        ContextPolicy::new(HistoryLimit::Unlimited, &loader),
    )
    .unwrap();
    assert!(
        regenerated
            .finalize_context()
            .unwrap_err()
            .message
            .contains("audio")
    );
}
