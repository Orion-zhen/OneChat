use super::*;

pub(super) fn parse_models(
    response: &Value,
    key: &str,
    kind: ProviderKind,
) -> Result<Vec<AvailableModel>, GenerationError> {
    response
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(invalid_model_list)
        .map(|models| {
            models
                .iter()
                .filter_map(|model| available_model(model, kind))
                .collect()
        })
}

pub(super) fn available_model(metadata: &Value, kind: ProviderKind) -> Option<AvailableModel> {
    let id = metadata.get("id").and_then(Value::as_str).or_else(|| {
        (kind == ProviderKind::Gemini)
            .then(|| metadata.get("name").and_then(Value::as_str))
            .flatten()
    })?;
    let id = id.strip_prefix("models/").unwrap_or(id).trim();
    (!id.is_empty()).then(|| AvailableModel {
        id: id.to_string(),
        tools: tools_from_metadata(metadata),
        vision: vision_from_metadata(metadata),
        audio: audio_from_metadata(metadata),
        context_window_tokens: context_window_from_metadata(metadata),
    })
}

pub(super) fn supports_gemini_generation(metadata: &Value) -> bool {
    metadata
        .get("supportedGenerationMethods")
        .and_then(Value::as_array)
        .is_none_or(|methods| {
            methods
                .iter()
                .filter_map(Value::as_str)
                .any(|method| matches!(method, "generateContent" | "streamGenerateContent"))
        })
}

fn tools_from_metadata(metadata: &Value) -> bool {
    tools_evidence(metadata).unwrap_or(false)
}

fn tools_evidence(value: &Value) -> Option<bool> {
    let Value::Object(object) = value else {
        return None;
    };
    let mut evidence = None;
    for (key, value) in object {
        let key = normalized_key(key);
        let direct = if matches!(
            key.as_str(),
            "tools"
                | "supportstools"
                | "toolcall"
                | "toolcalling"
                | "supportstoolcalling"
                | "tooluse"
                | "supportstooluse"
                | "functioncall"
                | "functioncalling"
                | "supportsfunctioncalling"
        ) {
            value.as_bool()
        } else if matches!(
            key.as_str(),
            "capabilities"
                | "features"
                | "supportedfeatures"
                | "supportedparameters"
                | "parameters"
        ) && value.is_array()
        {
            Some(contains_tool_label(value))
        } else {
            None
        };
        evidence = merge_evidence(evidence, direct);
        evidence = merge_evidence(evidence, tools_evidence(value));
    }
    evidence
}

fn contains_tool_label(value: &Value) -> bool {
    match value {
        Value::String(value) => matches!(
            normalized_key(value).as_str(),
            "tool"
                | "tools"
                | "toolcall"
                | "toolcalling"
                | "toolchoice"
                | "tooluse"
                | "paralleltoolcalls"
                | "function"
                | "functions"
                | "functioncall"
                | "functioncalling"
        ),
        Value::Array(values) => values.iter().any(contains_tool_label),
        Value::Object(values) => values.iter().any(|(key, value)| {
            contains_tool_label(&Value::String(key.clone())) && value.as_bool().unwrap_or(true)
        }),
        _ => false,
    }
}

fn vision_from_metadata(metadata: &Value) -> bool {
    vision_evidence(metadata).unwrap_or(false)
}

fn vision_evidence(value: &Value) -> Option<bool> {
    let Value::Object(object) = value else {
        return None;
    };
    let mut evidence = None;
    for (key, value) in object {
        let key = normalized_key(key);
        let direct = if matches!(
            key.as_str(),
            "vision" | "supportsvision" | "visioninput" | "imageinput" | "supportsimageinput"
        ) {
            value.as_bool()
        } else if matches!(
            key.as_str(),
            "modality"
                | "modalities"
                | "inputmodality"
                | "inputmodalities"
                | "supportedmodalities"
                | "supportedinputmodalities"
        ) || (matches!(
            key.as_str(),
            "capabilities" | "features" | "supportedfeatures"
        ) && value.is_array())
        {
            Some(contains_image_label(value))
        } else {
            None
        };
        evidence = merge_evidence(evidence, direct);
        evidence = merge_evidence(evidence, vision_evidence(value));
    }
    evidence
}

fn audio_from_metadata(metadata: &Value) -> bool {
    audio_evidence(metadata).unwrap_or(false)
}

fn audio_evidence(value: &Value) -> Option<bool> {
    let Value::Object(object) = value else {
        return None;
    };
    let mut evidence = None;
    for (key, value) in object {
        let key = normalized_key(key);
        let direct = if matches!(
            key.as_str(),
            "audio" | "supportsaudio" | "audioinput" | "inputaudio" | "supportsaudioinput"
        ) {
            value.as_bool()
        } else if matches!(
            key.as_str(),
            "modality"
                | "modalities"
                | "inputmodality"
                | "inputmodalities"
                | "supportedmodalities"
                | "supportedinputmodalities"
        ) || (matches!(
            key.as_str(),
            "capabilities" | "features" | "supportedfeatures"
        ) && value.is_array())
        {
            Some(contains_audio_label(value))
        } else {
            None
        };
        evidence = merge_evidence(evidence, direct);
        evidence = merge_evidence(evidence, audio_evidence(value));
    }
    evidence
}

fn context_window_from_metadata(value: &Value) -> Option<u32> {
    match value {
        Value::Object(object) => object
            .iter()
            .flat_map(|(key, value)| {
                let direct = matches!(
                    normalized_key(key).as_str(),
                    "contextlength"
                        | "contextwindow"
                        | "contextwindowsize"
                        | "maxcontextlength"
                        | "maxmodellen"
                        | "inputtokenlimit"
                        | "maxinputtokens"
                )
                .then(|| positive_u32(value))
                .flatten();
                direct
                    .into_iter()
                    .chain(context_window_from_metadata(value))
            })
            .max(),
        Value::Array(values) => values.iter().filter_map(context_window_from_metadata).max(),
        _ => None,
    }
}

fn positive_u32(value: &Value) -> Option<u32> {
    let value = match value {
        Value::Number(value) => value.as_u64().and_then(|value| u32::try_from(value).ok()),
        Value::String(value)
            if !value.is_empty() && value.chars().all(|character| character.is_ascii_digit()) =>
        {
            value.parse().ok()
        }
        _ => None,
    }?;
    (value > 0).then_some(value)
}

fn merge_evidence(current: Option<bool>, next: Option<bool>) -> Option<bool> {
    match (current, next) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), _) | (_, Some(false)) => Some(false),
        (None, None) => None,
    }
}

fn contains_image_label(value: &Value) -> bool {
    match value {
        Value::String(value) => {
            let value = value.to_ascii_lowercase();
            value.contains("image") || value.contains("vision")
        }
        Value::Array(values) => values.iter().any(contains_image_label),
        Value::Object(values) => values.iter().any(|(key, value)| {
            (key.eq_ignore_ascii_case("image") || key.eq_ignore_ascii_case("vision"))
                && value.as_bool().unwrap_or(true)
        }),
        _ => false,
    }
}

fn contains_audio_label(value: &Value) -> bool {
    match value {
        Value::String(value) => value.to_ascii_lowercase().contains("audio"),
        Value::Array(values) => values.iter().any(contains_audio_label),
        Value::Object(values) => values.iter().any(|(key, value)| {
            normalized_key(key).contains("audio") && value.as_bool().unwrap_or(true)
        }),
        _ => false,
    }
}

fn normalized_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(super) fn sorted_unique(models: Vec<AvailableModel>) -> Vec<AvailableModel> {
    models
        .into_iter()
        .fold(BTreeMap::new(), |mut models, model| {
            models
                .entry(model.id.clone())
                .and_modify(|stored: &mut AvailableModel| {
                    stored.tools |= model.tools;
                    stored.vision |= model.vision;
                    stored.audio |= model.audio;
                    stored.context_window_tokens = stored
                        .context_window_tokens
                        .max(model.context_window_tokens);
                })
                .or_insert(model);
            models
        })
        .into_values()
        .collect()
}

pub(super) fn invalid_model_list() -> GenerationError {
    GenerationError::new(
        GenerationErrorKind::Unknown,
        "Provider returned an invalid model list",
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_core_capabilities_from_model_metadata() {
        let model = available_model(
            &json!({
                "id": "multimodal-model",
                "architecture": {
                    "input_modalities": ["text", "image", "audio"]
                },
                "supported_parameters": ["tools"]
            }),
            ProviderKind::OpenAiCompatible,
        )
        .unwrap();

        assert!(model.vision);
        assert!(model.audio);
        assert!(model.tools);
    }

    #[test]
    fn parses_nested_audio_flags() {
        assert!(audio_from_metadata(&json!({
            "capabilities": { "supportsAudioInput": true }
        })));
        assert!(!audio_from_metadata(&json!({
            "capabilities": { "audioInput": false }
        })));
    }

    #[test]
    fn parses_context_windows_from_known_metadata_aliases() {
        let openrouter = available_model(
            &json!({ "id": "openrouter-model", "context_length": 131072 }),
            ProviderKind::OpenAiCompatible,
        )
        .unwrap();
        let gemini = available_model(
            &json!({ "name": "models/gemini-model", "inputTokenLimit": 1_000_000 }),
            ProviderKind::Gemini,
        )
        .unwrap();
        let nested = available_model(
            &json!({ "id": "nested-model", "metadata": { "limits": { "maxInputTokens": "65536" } } }),
            ProviderKind::OpenAiCompatible,
        )
        .unwrap();
        let vllm = available_model(
            &json!({ "id": "vllm-model", "max_model_len": 32768 }),
            ProviderKind::OpenAiCompatible,
        )
        .unwrap();

        assert_eq!(openrouter.context_window_tokens, Some(131_072));
        assert_eq!(gemini.context_window_tokens, Some(1_000_000));
        assert_eq!(nested.context_window_tokens, Some(65_536));
        assert_eq!(vllm.context_window_tokens, Some(32_768));
    }

    #[test]
    fn rejects_ambiguous_or_invalid_context_window_metadata() {
        for metadata in [
            json!({ "max_tokens": 128_000 }),
            json!({ "context_length": 0 }),
            json!({ "context_window": -1 }),
            json!({ "context_window_size": 128.0 }),
            json!({ "max_context_length": " 128000 " }),
            json!({ "inputTokenLimit": 4_294_967_296_u64 }),
            json!({ "id": "no-limit" }),
        ] {
            assert_eq!(context_window_from_metadata(&metadata), None);
        }
    }

    #[test]
    fn merges_metadata_from_duplicate_models() {
        let models = sorted_unique(vec![
            AvailableModel {
                id: "model".into(),
                tools: true,
                vision: false,
                audio: false,
                context_window_tokens: Some(32_000),
            },
            AvailableModel {
                id: "model".into(),
                tools: false,
                vision: true,
                audio: true,
                context_window_tokens: Some(128_000),
            },
        ]);

        assert_eq!(
            models,
            vec![AvailableModel {
                id: "model".into(),
                tools: true,
                vision: true,
                audio: true,
                context_window_tokens: Some(128_000),
            }]
        );
    }
}
