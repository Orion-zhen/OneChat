use super::*;
use onechat::application::context_usage::estimate_input_tokens;

#[test]
fn every_window_keeps_the_largest_complete_suffix_with_the_flat_request_estimate() {
    let provider = Provider::new("Test", ProviderKind::OpenAi);
    let mut model = Model::new(&provider.id, "test", "Test", provider.kind);
    model.capabilities.audio_input = true;
    let mut conversation = Conversation::new("Chat", Some(&model), "系");
    conversation.assistant_opening = "开场".into();
    let mut turns = Vec::new();
    let mut parent = None;
    for (index, duration) in [1, 33, 101].into_iter().enumerate() {
        let mut turn = completed_turn(
            &conversation,
            parent,
            &format!("问题{index}"),
            "answer",
            &model,
            &provider,
        );
        let mut audio = audio_attachment();
        audio.audio.as_mut().unwrap().duration_ms = duration;
        turn.user.attachments.push(audio);
        turn.responses[0].transcript = vec![
            Message::assistant(format!("call {index}")),
            Message::user(format!("result {index}")),
            Message::assistant("答"),
        ];
        parent = Some(turn.responses[0].id.clone());
        turns.push(turn);
    }
    let loader = |user: &UserMessage| Ok(Message::user(user.content.clone()));
    let prepare = |limit| {
        PreparedGeneration::new(
            &conversation,
            &provider,
            &model,
            &turns,
            parent.clone(),
            UserMessage::new("current", Vec::new()),
            ContextPolicy::new(limit, &loader),
        )
        .unwrap()
    };
    let candidates = (0..=turns.len())
        .map(|keep| {
            let prepared = prepare(HistoryLimit::Last(keep as u32));
            let request = prepared.request.into_request();
            let tokens = estimate_input_tokens(
                &request.system_prompt,
                &request.messages,
                request.audio_duration_ms,
            );
            assert_eq!(prepared.request_info.usage.input_tokens, Some(tokens));
            (request, tokens)
        })
        .collect::<Vec<_>>();
    let full = prepare(HistoryLimit::Unlimited);
    for window in 0..=candidates.last().unwrap().1 + 1 {
        let keep = (0..candidates.len())
            .rev()
            .find(|index| candidates[*index].1 <= window)
            .unwrap_or(0);
        let mut prepared = full.clone();
        prepared.request.model.context_window_tokens = Some(window as u32);
        prepared.finalize_context().unwrap();
        let context = prepared.request_info.context.unwrap();
        assert_eq!(context.available_history_turns, 3);
        assert_eq!(context.included_history_turns, keep as u32);
        assert_eq!(context.limited_by_context_window, keep < turns.len());
        assert_eq!(
            prepared.request_info.usage.input_tokens,
            Some(candidates[keep].1)
        );
        let request = prepared.request.into_request();
        assert_eq!(request.messages, candidates[keep].0.messages);
        assert_eq!(
            request.audio_duration_ms,
            candidates[keep].0.audio_duration_ms
        );
    }
}

#[tokio::test]
async fn resolved_opening_and_complete_continuation_tail_survive_removing_all_history() {
    let provider = Provider::new("Test", ProviderKind::OpenAi);
    let mut model = Model::new(&provider.id, "test", "Test", provider.kind);
    model.capabilities.audio_input = true;
    model.context_window_tokens = Some(1);
    let mut conversation = Conversation::new("Chat", Some(&model), "system");
    conversation.assistant_opening = "{{opening}}".into();
    let root = completed_turn(
        &conversation,
        None,
        "old question",
        "old answer",
        &model,
        &provider,
    );
    let mut current = completed_turn(
        &conversation,
        Some(root.responses[0].id.clone()),
        "current question",
        "partial answer",
        &model,
        &provider,
    );
    current.user.attachments.push(audio_attachment());
    let transcript = vec![
        Message::assistant("tool call"),
        Message::user("tool result"),
        Message::assistant("partial answer"),
    ];
    current.responses[0].transcript = transcript.clone();
    let mut prepared = PreparedGeneration::continuation(
        &conversation,
        &provider,
        &model,
        &[root, current.clone()],
        &current,
        &current.responses[0],
        ContextPolicy::new(HistoryLimit::Unlimited, &|user| {
            Ok(Message::user(user.content.clone()))
        }),
    )
    .unwrap();
    let opening = "expanded opening ".repeat(20).trim().to_string();
    prepared.configure_prompt(
        BTreeMap::from([(
            "opening".into(),
            PromptVariableSource::Text {
                value: opening.clone(),
            },
        )]),
        Default::default(),
    );
    prepared
        .render_prompt_setup(CancellationToken::new())
        .await
        .unwrap();
    prepared.finalize_context().unwrap();
    assert_eq!(
        prepared
            .request_info
            .context
            .unwrap()
            .included_history_turns,
        0
    );
    assert_eq!(
        prepared
            .request_info
            .assistant_opening
            .as_ref()
            .unwrap()
            .resolved,
        opening
    );
    let request = prepared.request.into_request();
    let mut expected = vec![
        Message::assistant(opening),
        Message::user("current question"),
    ];
    expected.extend(transcript);
    assert_eq!(request.messages, expected);
    assert_eq!(request.audio_duration_ms, 1_000);
    assert_eq!(
        prepared.request_info.usage.input_tokens,
        Some(estimate_input_tokens("system", &request.messages, 1_000))
    );
}
