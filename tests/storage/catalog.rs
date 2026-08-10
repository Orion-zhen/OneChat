use super::*;

#[test]
fn catalog_settings_and_prompt_presets_round_trip() {
    let (_directory, storage) = open_storage();
    fs::write(
        storage.settings_path(),
        "{\n  // JSONC is accepted\n  providers: [],\n  models: [],\n}\n",
    )
    .unwrap();
    assert!(
        storage
            .load_snapshot()
            .unwrap()
            .settings
            .parse_document_images
    );

    let (provider, model) = catalog(&storage);
    let conversation = Conversation::new("First chat", Some(&model), "");
    storage.insert_conversation(&conversation).unwrap();
    let mut settings = AppSettings {
        current_conversation_id: Some(conversation.id.clone()),
        primary_model_id: Some(model.id.clone()),
        title_generation_model_id: Some(model.id.clone()),
        title_generation_reasoning_preset: Some("low".into()),
        theme_color: "#AF52DE".into(),
        code_block_wrap: true,
        parse_document_images: false,
        ..AppSettings::default()
    };
    settings.prompt_variables.insert(
        "owner".into(),
        PromptVariableSource::Text {
            value: "Orion".into(),
        },
    );
    settings.prompt_variables.insert(
        "repo".into(),
        PromptVariableSource::Command {
            script: "git status --short".into(),
            cwd: Some("/tmp/repo".into()),
            timeout_ms: 1_500,
        },
    );
    storage.save_settings(&settings).unwrap();

    storage
        .insert_prompt_preset(&SystemPromptPreset::new("Concise", " Be brief. "))
        .unwrap();
    storage
        .update_prompt_preset(
            "Concise",
            &SystemPromptPreset::new("Direct", "Answer directly."),
        )
        .unwrap();

    let snapshot = storage.load_snapshot().unwrap();
    assert_eq!(snapshot.providers, vec![provider.clone()]);
    assert_eq!(snapshot.models, vec![model.clone()]);
    assert_eq!(snapshot.conversations, vec![conversation]);
    assert_eq!(
        snapshot.prompt_presets,
        vec![SystemPromptPreset::new("Direct", "Answer directly.")]
    );
    assert_eq!(snapshot.settings.primary_model_id, Some(model.id.clone()));
    assert!(snapshot.settings.code_block_wrap);
    assert!(!snapshot.settings.parse_document_images);
    assert_eq!(snapshot.settings.theme_color, "#AF52DE");
    assert_eq!(
        snapshot.settings.prompt_variables,
        settings.prompt_variables
    );
    assert_eq!(
        snapshot
            .settings
            .title_generation_reasoning_preset
            .as_deref(),
        Some("low")
    );

    storage.delete_provider(&provider.id).unwrap();
    let snapshot = storage.load_snapshot().unwrap();
    assert!(snapshot.providers.is_empty());
    assert!(snapshot.models.is_empty());
    assert_eq!(snapshot.conversations[0].model_id, None);
    assert_eq!(snapshot.settings.primary_model_id, None);
    assert_eq!(snapshot.settings.title_generation_model_id, None);

    storage.delete_prompt_preset("Direct").unwrap();
    settings.current_conversation_id = None;
    storage.save_settings(&settings).unwrap();
    assert!(storage.load_snapshot().unwrap().prompt_presets.is_empty());
}
