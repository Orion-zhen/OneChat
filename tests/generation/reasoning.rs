use super::*;

#[test]
fn model_id_presets_resolve_without_request_parameters() {
    let config: ModelReasoningConfig = serde_json::from_value(json!({
        "type": "model_id_suffix",
        "default_preset": "high",
        "presets": [
            {"level": null, "model_id": "model"},
            {"level": "off", "model_id": "model:OFF"},
            {"level": "high", "model_id": "model:High"}
        ]
    }))
    .unwrap();
    config.validate().unwrap();
    assert_eq!(
        config.preset_options(),
        vec![
            ("provider_default".into(), "Default".into()),
            ("off".into(), "Off".into()),
            ("high".into(), "High".into()),
        ]
    );
    for (selected, effective, expected_model) in [
        (None, "high", "model:High"),
        (Some("removed-preset"), "high", "model:High"),
        (Some("off"), "off", "model:OFF"),
        (Some("provider_default"), "provider_default", "model"),
    ] {
        assert_eq!(
            config.resolve_patch(selected).unwrap(),
            (effective.into(), Map::new())
        );
        assert_eq!(
            config.resolve_model_id("model", selected).unwrap(),
            expected_model
        );
    }
}

#[test]
fn suffix_only_presets_do_not_offer_a_nonexistent_base_model() {
    let config: ModelReasoningConfig = serde_json::from_value(json!({
        "type": "model_id_suffix",
        "default_preset": "on",
        "presets": [{"level": "on", "model_id": "model:ON"}]
    }))
    .unwrap();
    config.validate().unwrap();
    assert_eq!(config.preset_options(), vec![("on".into(), "On".into())]);
    assert_eq!(
        config
            .resolve_model_id("model", Some("provider_default"))
            .unwrap(),
        "model:ON"
    );
}

#[test]
fn validates_stored_model_id_presets() {
    for value in [
        json!({"default_preset": "high", "presets": []}),
        json!({"default_preset": "provider_default", "presets": [{"level": "high", "model_id": "model:high"}]}),
        json!({"default_preset": "high", "presets": [{"level": "high", "model_id": ""}]}),
        json!({"default_preset": "high", "presets": [{"level": "high", "model_id": "model:low"}]}),
        json!({"default_preset": "high", "presets": [{"level": "high", "model_id": "model:high"}, {"level": "high", "model_id": "model:HIGH"}]}),
    ] {
        let mut value = value;
        value["type"] = json!("model_id_suffix");
        let config: ModelReasoningConfig = serde_json::from_value(value).unwrap();
        assert!(config.validate().is_err());
    }
}

#[test]
fn reasoning_presets_compile_and_merge_into_request_parameters() {
    let known = ModelReasoningConfig::known(KnownReasoningFormat::AnthropicManualBudget);
    let (_, patch) = known.resolve_patch(Some("high")).unwrap();
    assert_eq!(
        Value::Object(patch),
        json!({"thinking": {"type": "enabled", "budget_tokens": 16384}})
    );

    let custom = ModelReasoningConfig::Custom {
        default_preset: "fast".into(),
        presets: vec![CustomReasoningPreset {
            id: "fast".into(),
            name: None,
            request_parameters: vec![ReasoningParameter {
                path: "reasoning.effort".into(),
                value: ReasoningParameterValue::String("low".into()),
            }],
            chat_template_kwargs: vec![ReasoningParameter {
                path: "thinking".into(),
                value: ReasoningParameterValue::Boolean(true),
            }],
        }],
    };
    custom.validate().unwrap();
    let (_, patch) = custom.resolve_patch(None).unwrap();
    assert_eq!(
        Value::Object(patch.clone()),
        json!({
            "reasoning": {"effort": "low"},
            "chat_template_kwargs": {"thinking": true}
        })
    );

    let mut request = Map::from_iter([
        ("temperature".into(), json!(0.8)),
        ("reasoning".into(), json!({"summary": "auto"})),
    ]);
    merge_json_patch(&mut request, patch);
    assert_eq!(request["temperature"], json!(0.8));
    assert_eq!(
        request["reasoning"],
        json!({"summary": "auto", "effort": "low"})
    );
}
