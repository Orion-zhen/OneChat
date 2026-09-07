use super::*;

#[test]
fn catalog_updates_preserve_live_chat_and_unrelated_settings() {
    let (_directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let conversation = Conversation::new("Live chat", Some(&model), "prompt");
    storage.insert_conversation(&conversation).unwrap();
    storage
        .save_settings(&AppSettings {
            current_conversation_id: Some(conversation.id.clone()),
            primary_model_id: Some(model.id.clone()),
            title_generation_model: TitleModelSource::Model(model.id.clone()),
            ..Default::default()
        })
        .unwrap();
    let mut prepared = prepare_turn(
        &storage,
        &conversation,
        &provider,
        &model,
        &[],
        None,
        UserMessage::new("question", Vec::new()),
    );
    let mut snapshot = storage.load_startup_snapshot().unwrap();
    let session = storage
        .update_session(&conversation.id, |session| {
            session.begin_generation(&prepared.start, &prepared.response, &prepared.request_info)
        })
        .unwrap();
    snapshot.apply_conversation(session, Some(&conversation.id));
    prepared.response.append_output("not yet persisted", 0);
    snapshot
        .current
        .as_mut()
        .unwrap()
        .update_generation(&prepared.response, &prepared.request_info)
        .unwrap();
    snapshot.settings.translation_system_prompt = "unsaved translation prompt".into();
    snapshot.settings.history_limit = HistoryLimit::Last(7);
    let mut expected_settings = snapshot.settings.clone();
    let mut expected_current = snapshot.current.clone().unwrap();
    let search = snapshot
        .conversation_search
        .entries(&conversation.id)
        .to_vec();

    let zulu = Model::new(&provider.id, "zulu", "Zulu", provider.kind);
    snapshot.apply_model_catalog(storage.insert_model(&zulu).unwrap());
    let alpha = Model::new(&provider.id, "alpha", "alpha", provider.kind);
    snapshot.apply_model_catalog(storage.insert_model(&alpha).unwrap());
    assert_eq!(snapshot.models, vec![alpha, model.clone(), zulu]);
    assert_eq!(snapshot.settings, expected_settings);
    assert_eq!(snapshot.current.as_ref(), Some(&expected_current));

    snapshot.apply_model_catalog(storage.delete_model(&model.id).unwrap());
    expected_current.conversation.model_id = None;
    expected_settings.primary_model_id = None;
    expected_settings.title_generation_model = TitleModelSource::Current;
    assert_eq!(snapshot.current.as_ref(), Some(&expected_current));
    assert_eq!(snapshot.conversations[0].model_id, None);
    assert_eq!(snapshot.settings, expected_settings);
    let entries = snapshot.conversation_search.entries(&conversation.id);
    assert_eq!(entries.len(), search.len());
    for (entry, original) in entries.iter().zip(&search) {
        assert_eq!(entry.turn_id, original.turn_id);
        assert_eq!(entry.response_id, original.response_id);
        assert_eq!(entry.source, original.source);
        assert_eq!(entry.content, original.content);
    }
    assert_eq!(
        storage
            .load_conversation(&conversation.id)
            .unwrap()
            .conversation
            .model_id,
        None
    );

    snapshot.apply_model_catalog(storage.delete_provider(&provider.id).unwrap());
    assert!(snapshot.models.is_empty());
    assert!(snapshot.providers.is_empty());
    assert_eq!(snapshot.current.as_ref(), Some(&expected_current));
}

#[test]
fn catalog_and_prompt_updates_do_not_reload_unrelated_data() {
    let (_directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let conversation = Conversation::new("Chat", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();
    let path = storage
        .conversations_dir()
        .join(&conversation.id)
        .join(format!("{}.json", conversation.id));
    let mut invalid = storage.load_conversation(&conversation.id).unwrap();
    invalid.conversation.id = "wrong-directory".into();
    fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
    let broken_prompt = storage.prompts_dir().join("Broken");
    fs::create_dir(&broken_prompt).unwrap();

    let mut changed_provider = provider.clone();
    changed_provider.enabled = false;
    assert_eq!(
        storage
            .update_provider(&changed_provider)
            .unwrap()
            .providers,
        vec![changed_provider]
    );
    let mut changed_model = model.clone();
    changed_model.display_name = "Renamed model".into();
    assert_eq!(
        storage.update_model(&changed_model).unwrap().models,
        vec![changed_model]
    );
    assert!(
        storage
            .delete_provider(&provider.id)
            .unwrap()
            .models
            .is_empty()
    );
    assert_eq!(
        storage
            .load_conversation(&conversation.id)
            .unwrap()
            .conversation
            .model_id,
        None
    );

    fs::remove_dir(&broken_prompt).unwrap();
    fs::write(storage.settings_path(), "invalid settings").unwrap();
    let preset = storage
        .insert_prompt_preset(&PromptPreset::new("Original", "prompt", ""))
        .unwrap();
    assert_eq!(storage.load_prompt_presets().unwrap(), vec![preset]);
    let renamed = storage
        .update_prompt_preset(
            "Original",
            &PromptPreset::new("Renamed", "new prompt", "opening"),
        )
        .unwrap();
    assert_eq!(storage.load_prompt_presets().unwrap(), vec![renamed]);
    storage.delete_prompt_preset("Renamed").unwrap();
    assert!(storage.load_prompt_presets().unwrap().is_empty());
}

#[test]
fn prompt_updates_change_only_presets_and_their_default_selection() {
    let (_directory, storage) = open_storage();
    let conversation = Conversation::new("Chat", None, "conversation prompt");
    storage.insert_conversation(&conversation).unwrap();
    storage
        .save_settings(&AppSettings {
            current_conversation_id: Some(conversation.id.clone()),
            default_prompt_preset: Some("Original".into()),
            ..Default::default()
        })
        .unwrap();
    let mut snapshot = storage.load_startup_snapshot().unwrap();
    let current = snapshot.current.clone();
    let mut settings = snapshot.settings.clone();
    snapshot.update_prompt_preset(None, PromptPreset::new("Zulu", "z", ""));
    snapshot.update_prompt_preset(None, PromptPreset::new("Original", "old", ""));
    snapshot.update_prompt_preset(
        Some("Original"),
        PromptPreset::new("alpha", "new", "opening"),
    );
    assert_eq!(
        snapshot
            .prompt_presets
            .iter()
            .map(|preset| preset.name.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "Zulu"]
    );
    settings.default_prompt_preset = Some("alpha".into());
    assert_eq!(snapshot.settings, settings);
    assert_eq!(snapshot.current, current);

    snapshot.remove_prompt_preset("alpha");
    settings.default_prompt_preset = None;
    assert_eq!(snapshot.settings, settings);
    assert_eq!(snapshot.current, current);
    assert_eq!(
        snapshot.prompt_presets,
        vec![PromptPreset::new("Zulu", "z", "")]
    );
}

#[test]
fn later_settings_save_cannot_restore_a_deleted_model_reference() {
    let (_directory, storage) = open_storage();
    let (_, model) = catalog(&storage);
    let stale = AppSettings {
        primary_model_id: Some(model.id.clone()),
        title_generation_model: TitleModelSource::Model(model.id.clone()),
        ..Default::default()
    };
    storage.delete_model(&model.id).unwrap();
    storage.save_settings(&stale).unwrap();
    let saved: AppSettings =
        serde_json::from_slice(&fs::read(storage.settings_path()).unwrap()).unwrap();
    assert_eq!(saved.primary_model_id, None);
    assert_eq!(saved.title_generation_model, TitleModelSource::Current);
}
