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
    assert!(model.audio_input);
    assert!(model.tools);
}

#[test]
fn parses_nested_audio_input_evidence() {
    assert!(audio_input_from_metadata(&json!({
        "capabilities": { "supportsAudioInput": true }
    })));
    assert!(audio_input_from_metadata(&json!({
        "architecture": {
            "input_modalities": ["text", "image", "audio"],
            "output_modalities": ["text"]
        }
    })));
    assert!(!audio_input_from_metadata(&json!({
        "capabilities": { "audioInput": false }
    })));
}

#[test]
fn ignores_output_only_and_ambiguous_audio_metadata() {
    for metadata in [
        json!({ "audio": true }),
        json!({ "supportsAudio": true }),
        json!({ "modalities": ["text", "audio"] }),
        json!({ "supportedModalities": ["audio"] }),
        json!({ "features": ["audio"] }),
        json!({ "outputModalities": ["audio"] }),
        json!({ "capabilities": { "audioTranscriptions": true } }),
        json!({ "modality": "text+image+audio->text" }),
    ] {
        assert!(!audio_input_from_metadata(&metadata), "{metadata}");
    }
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
            audio_input: false,
            context_window_tokens: Some(32_000),
        },
        AvailableModel {
            id: "model".into(),
            tools: false,
            vision: true,
            audio_input: true,
            context_window_tokens: Some(128_000),
        },
    ]);

    assert_eq!(
        models,
        vec![AvailableModel {
            id: "model".into(),
            tools: true,
            vision: true,
            audio_input: true,
            context_window_tokens: Some(128_000),
        }]
    );
}
