use super::*;

#[test]
fn model_id_reasoning_presets_and_default_survive_storage_round_trip() {
    use onechat::domain::{
        ModelIdReasoningConfig, ModelIdReasoningPreset, ModelReasoningConfig, ReasoningLevel,
    };

    let (_directory, storage) = open_storage();
    let (_, mut model) = catalog(&storage);
    model.reasoning = Some(ModelReasoningConfig::ModelIdSuffix(
        ModelIdReasoningConfig {
            default_preset: "high".into(),
            presets: vec![
                ModelIdReasoningPreset {
                    level: None,
                    model_id: model.remote_id.clone(),
                },
                ModelIdReasoningPreset {
                    level: Some(ReasoningLevel::High),
                    model_id: format!("{}:HIGH", model.remote_id),
                },
            ],
        },
    ));
    storage.update_model(&model).unwrap();
    let saved = storage.load_startup_snapshot().unwrap().models.remove(0);
    assert_eq!(saved.reasoning, model.reasoning);
    let config = saved.reasoning.unwrap();
    assert_eq!(config.default_preset(), "high");
    assert_eq!(
        config.resolve_model_id(&saved.remote_id, None).unwrap(),
        format!("{}:HIGH", model.remote_id)
    );
}

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
            .load_startup_snapshot()
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
        title_generation_model: TitleModelSource::Model(model.id.clone()),
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
        .insert_prompt_preset(&PromptPreset::new(
            "Concise",
            " Be brief. ",
            "Welcome, {{owner}}.",
        ))
        .unwrap();
    storage
        .update_prompt_preset(
            "Concise",
            &PromptPreset::new("Direct", "Answer directly.", "How can I help?"),
        )
        .unwrap();
    let prompts = storage.settings_path().parent().unwrap().join("prompts");
    assert!(!prompts.join("Concise").exists());
    assert_eq!(
        fs::read_to_string(prompts.join("Direct/Direct.md")).unwrap(),
        "Answer directly.\n"
    );
    assert_eq!(
        fs::read_to_string(prompts.join("Direct/Direct.opening.md")).unwrap(),
        "How can I help?\n"
    );

    let snapshot = storage.load_startup_snapshot().unwrap();
    assert_eq!(snapshot.providers, vec![provider.clone()]);
    assert_eq!(snapshot.models, vec![model.clone()]);
    assert_eq!(snapshot.conversations, vec![conversation]);
    assert_eq!(
        snapshot.prompt_presets,
        vec![PromptPreset::new(
            "Direct",
            "Answer directly.",
            "How can I help?"
        )]
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

    storage
        .update_prompt_preset(
            "Direct",
            &PromptPreset::new("Direct", "Answer directly.", ""),
        )
        .unwrap();
    assert!(!prompts.join("Direct/Direct.opening.md").exists());
    assert_eq!(
        storage.load_prompt_presets().unwrap()[0].assistant_opening,
        ""
    );

    storage.delete_provider(&provider.id).unwrap();
    let snapshot = storage.load_startup_snapshot().unwrap();
    assert!(snapshot.providers.is_empty());
    assert!(snapshot.models.is_empty());
    assert_eq!(snapshot.conversations[0].model_id, None);
    assert_eq!(snapshot.settings.primary_model_id, None);
    assert_eq!(
        snapshot.settings.title_generation_model,
        TitleModelSource::Current
    );

    storage.delete_prompt_preset("Direct").unwrap();
    settings.current_conversation_id = None;
    storage.save_settings(&settings).unwrap();
    assert!(
        storage
            .load_startup_snapshot()
            .unwrap()
            .prompt_presets
            .is_empty()
    );
}
