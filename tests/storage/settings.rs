use super::*;

#[test]
fn model_context_window_persists_and_can_be_cleared() {
    let (_directory, storage) = open_storage();
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    storage.insert_provider(&provider).unwrap();
    let mut model = Model::new(&provider.id, "model", "Model", provider.kind);
    model.context_window_tokens = Some(128_000);
    storage.insert_model(&model).unwrap();

    assert_eq!(
        storage.load_startup_snapshot().unwrap().models[0].context_window_tokens,
        Some(128_000)
    );

    model.context_window_tokens = None;
    storage.update_model(&model).unwrap();
    assert_eq!(
        storage.load_startup_snapshot().unwrap().models[0].context_window_tokens,
        None
    );
}

#[test]
fn storage_normalizes_out_of_range_history_limit() {
    let (_directory, storage) = open_storage();
    fs::write(
        storage.settings_path(),
        r#"{
            providers: [],
            models: [],
            history_limit: { mode: "last", turns: 999 },
        }"#,
    )
    .unwrap();

    let snapshot = storage.load_startup_snapshot().unwrap();
    assert_eq!(snapshot.settings.history_limit, HistoryLimit::Last(50));
    assert_eq!(
        storage
            .load_startup_snapshot()
            .unwrap()
            .settings
            .history_limit,
        HistoryLimit::Last(50)
    );
}

#[test]
fn global_history_limit_values_round_trip() {
    let (_directory, storage) = open_storage();
    let mut settings = storage.load_startup_snapshot().unwrap().settings;

    for limit in [
        HistoryLimit::Last(0),
        HistoryLimit::Last(1),
        HistoryLimit::Last(50),
        HistoryLimit::Unlimited,
    ] {
        settings.history_limit = limit;
        storage.save_settings(&settings).unwrap();
        assert_eq!(
            storage
                .load_startup_snapshot()
                .unwrap()
                .settings
                .history_limit,
            limit
        );
    }
}

#[test]
fn conversation_history_override_is_explicit_until_reset() {
    let (_directory, storage) = open_storage();
    let mut settings = storage.load_startup_snapshot().unwrap().settings;
    settings.history_limit = HistoryLimit::Last(8);
    storage.save_settings(&settings).unwrap();
    let mut conversation = Conversation::new("Chat", None, "");
    storage.insert_conversation(&conversation).unwrap();

    assert_eq!(
        conversation.effective_history_limit(settings.history_limit),
        HistoryLimit::Last(8)
    );
    settings.history_limit = HistoryLimit::Last(3);
    storage.save_settings(&settings).unwrap();
    assert_eq!(
        conversation.effective_history_limit(settings.history_limit),
        HistoryLimit::Last(3)
    );

    conversation.history_limit_override = Some(HistoryLimit::Last(3));
    storage.update_conversation(&conversation).unwrap();
    settings.history_limit = HistoryLimit::Last(1);
    storage.save_settings(&settings).unwrap();
    let stored = storage
        .load_conversation(&conversation.id)
        .unwrap()
        .conversation;
    assert_eq!(stored.history_limit_override, Some(HistoryLimit::Last(3)));
    assert_eq!(
        stored.effective_history_limit(settings.history_limit),
        HistoryLimit::Last(3)
    );

    conversation.history_limit_override = Some(HistoryLimit::Unlimited);
    storage.update_conversation(&conversation).unwrap();
    let stored = storage
        .load_conversation(&conversation.id)
        .unwrap()
        .conversation;
    assert_eq!(
        stored.effective_history_limit(settings.history_limit),
        HistoryLimit::Unlimited
    );

    conversation.history_limit_override = None;
    storage.update_conversation(&conversation).unwrap();
    let stored = storage
        .load_conversation(&conversation.id)
        .unwrap()
        .conversation;
    assert_eq!(stored.history_limit_override, None);
    assert_eq!(
        stored.effective_history_limit(settings.history_limit),
        HistoryLimit::Last(1)
    );
}

#[test]
fn window_state_round_trips() {
    let (_directory, storage) = open_storage();
    assert_eq!(storage.load_window_state().unwrap(), None);

    let state = WindowState {
        mode: WindowMode::Maximized,
        display: Some("display-id".into()),
        x: 120.0,
        y: 80.0,
        width: 1380.0,
        height: 900.0,
    };
    storage.save_window_state(&state).unwrap();

    assert_eq!(storage.load_window_state().unwrap(), Some(state));
}

#[test]
fn provider_order_round_trips() {
    let (_directory, storage) = open_storage();
    let providers = ["Zulu", "Alpha", "Middle"].map(|name| {
        let provider = Provider::new(name, ProviderKind::OpenAi);
        storage.insert_provider(&provider).unwrap();
        provider
    });

    assert_eq!(
        storage
            .load_startup_snapshot()
            .unwrap()
            .providers
            .iter()
            .map(|provider| provider.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Zulu", "Alpha", "Middle"]
    );

    let ordered_ids = [2, 0, 1].map(|index| providers[index].id.clone()).to_vec();
    storage.reorder_providers(&ordered_ids).unwrap();

    assert_eq!(
        storage
            .load_startup_snapshot()
            .unwrap()
            .providers
            .iter()
            .map(|provider| provider.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Middle", "Zulu", "Alpha"]
    );
}
