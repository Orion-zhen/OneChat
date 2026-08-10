use super::*;

#[test]
fn new_turn_history_follows_the_selected_branch() {
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
    let root_response_id = root.responses[0].id.clone();
    let mut old_branch = completed_turn(
        &conversation,
        Some(root_response_id.clone()),
        "old branch",
        "old answer",
        &model,
        &provider,
    );
    old_branch.selected = false;
    let selected_branch = completed_turn(
        &conversation,
        Some(root_response_id),
        "selected branch",
        "selected answer",
        &model,
        &provider,
    );

    let history = history_for_new_turn(
        &[root, old_branch, selected_branch],
        HistoryLimit::Unlimited,
    );
    let messages = serialized_messages(&history);
    assert_eq!(messages.len(), 4);
    assert!(messages[0].contains("root question"));
    assert!(messages[1].contains("root answer"));
    assert!(messages[2].contains("selected branch"));
    assert!(messages[3].contains("selected answer"));
    assert!(
        messages
            .iter()
            .all(|message| !message.contains("old branch"))
    );
}

#[test]
fn history_limits_keep_recent_complete_turns_and_do_not_count_current_message() {
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
    let middle = completed_turn(
        &conversation,
        Some(root.responses[0].id.clone()),
        "middle question",
        "middle answer",
        &model,
        &provider,
    );
    let turns = [root, middle.clone()];
    let loader = |user: &UserMessage| {
        if user.content == "root question" {
            Err("excluded root was expanded".into())
        } else {
            Ok(Message::user(user.content.clone()))
        }
    };

    let prepared = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(middle.responses[0].id.clone()),
        UserMessage::new("current question", Vec::new()),
        ContextPolicy::new(HistoryLimit::Last(1), &loader),
    )
    .unwrap();
    let messages = serialized_messages(&prepared.provider_request.messages);
    assert_eq!(messages.len(), 3);
    assert!(messages[0].contains("middle question"));
    assert!(messages[1].contains("middle answer"));
    assert!(messages[2].contains("current question"));
    assert!(messages.iter().all(|message| !message.contains("root")));
    let context = prepared.request_info.context.unwrap();
    assert_eq!(context.history_limit, HistoryLimit::Last(1));
    assert_eq!(context.available_history_turns, 2);
    assert_eq!(context.included_history_turns, 1);

    let stateless = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        &turns,
        Some(middle.responses[0].id.clone()),
        UserMessage::new("current question", Vec::new()),
        ContextPolicy::new(HistoryLimit::Last(0), &|user| {
            Ok(Message::user(user.content.clone()))
        }),
    )
    .unwrap();
    let messages = serialized_messages(&stateless.provider_request.messages);
    assert_eq!(messages.len(), 1);
    assert!(messages[0].contains("current question"));

    let oversized_limit = history_for_new_turn(&turns, HistoryLimit::Last(50));
    assert_eq!(oversized_limit.len(), 4);
    let preview = history_preview_for_new_turn(&turns, HistoryLimit::Last(1));
    assert_eq!(preview.available_turns, 2);
    assert_eq!(preview.included_turns, 1);
}

#[test]
fn history_turns_keep_complete_transcripts_as_the_truncation_unit() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "test-model", "Test Model");
    let conversation = Conversation::new("Chat", Some(&model), "");
    let mut root = completed_turn(
        &conversation,
        None,
        "tool question",
        "fallback answer",
        &model,
        &provider,
    );
    root.responses[0].transcript = vec![
        Message::assistant("tool call marker"),
        Message::user("tool result marker"),
    ];

    let included = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        std::slice::from_ref(&root),
        Some(root.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Last(1), &|user| {
            Ok(Message::user(user.content.clone()))
        }),
    )
    .unwrap();
    let included = serialized_messages(&included.provider_request.messages).join("\n");
    assert!(included.contains("tool question"));
    assert!(included.contains("tool call marker"));
    assert!(included.contains("tool result marker"));
    assert!(!included.contains("fallback answer"));

    let excluded = PreparedGeneration::new(
        &conversation,
        &provider,
        &model,
        std::slice::from_ref(&root),
        Some(root.responses[0].id.clone()),
        UserMessage::new("current", Vec::new()),
        ContextPolicy::new(HistoryLimit::Last(0), &|user| {
            Ok(Message::user(user.content.clone()))
        }),
    )
    .unwrap();
    let excluded = serialized_messages(&excluded.provider_request.messages).join("\n");
    assert!(!excluded.contains("tool question"));
    assert!(!excluded.contains("tool call marker"));
    assert!(!excluded.contains("tool result marker"));
}
