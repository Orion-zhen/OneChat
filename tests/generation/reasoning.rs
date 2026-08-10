use super::*;

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
