use super::*;
use crate::domain::{GenerationConfig, Model};

fn request(kind: ProviderKind, audio_input: bool, with_audio: bool) -> GenerationRequest {
    let provider = Provider::new("Provider", kind);
    let mut model = Model::new(&provider.id, "model", "Model");
    model.capabilities.audio_input = audio_input;
    let messages = if with_audio {
        vec![Message::User {
            content: vec![
                rig_core::message::UserContent::text("Listen"),
                rig_core::message::UserContent::audio(
                    "UklGRg==",
                    Some(rig_core::message::AudioMediaType::WAV),
                ),
            ],
        }]
    } else {
        vec![Message::user("Hello")]
    };
    GenerationRequest {
        provider,
        model,
        system_prompt: String::new(),
        config: GenerationConfig::default(),
        messages,
        audio_duration_ms: 0,
        tools: Vec::new(),
    }
}

#[test]
fn chat_completions_serializes_wav_and_mp3_as_ordered_input_audio() {
    use rig_core::message::{AudioMediaType, UserContent};

    let content = [
        UserContent::text("First"),
        UserContent::audio("d2F2", Some(AudioMediaType::WAV)),
        UserContent::text("Second"),
        UserContent::audio("bXAz", Some(AudioMediaType::MP3)),
    ]
    .into_iter()
    .map(rig_openai::completion::UserContent::try_from)
    .collect::<Result<Vec<_>, _>>()
    .unwrap();
    let value = serde_json::to_value(content).unwrap();

    assert_eq!(value[0], json!({ "type": "text", "text": "First" }));
    assert_eq!(
        value[1],
        json!({
            "type": "input_audio",
            "input_audio": { "data": "d2F2", "format": "wav" }
        })
    );
    assert_eq!(value[2], json!({ "type": "text", "text": "Second" }));
    assert_eq!(
        value[3],
        json!({
            "type": "input_audio",
            "input_audio": { "data": "bXAz", "format": "mp3" }
        })
    );
    assert!(!value.to_string().contains("modalities"));
}

#[test]
fn local_audio_duration_metadata_is_not_in_the_sdk_request() {
    let mut request = request(ProviderKind::OpenAi, true, true);
    request.audio_duration_ms = 12_345;
    let sdk_request = sdk_request(&request, Map::new()).unwrap();
    let value = serde_json::to_string(&sdk_request).unwrap();
    assert!(!value.contains("audio_duration"));
    assert!(!value.contains("12345"));
}

#[test]
fn input_audio_never_enables_audio_output_parameters() {
    let mut request = request(ProviderKind::OpenAi, true, true);
    request
        .config
        .extra
        .insert("modalities".into(), json!(["text", "audio"]));
    request
        .config
        .extra
        .insert("audio".into(), json!({ "voice": "alloy", "format": "wav" }));
    let parameters = additional_parameters(&request).unwrap();
    assert!(!parameters.contains_key("modalities"));
    assert!(!parameters.contains_key("audio"));
}

#[test]
fn native_openai_route_depends_only_on_model_audio_capability() {
    assert_eq!(
        request_api(&request(ProviderKind::OpenAi, true, false)),
        OpenAiApi::ChatCompletions
    );
    assert_eq!(
        request_api(&request(ProviderKind::OpenAi, true, true)),
        OpenAiApi::ChatCompletions
    );
    assert_eq!(
        request_api(&request(ProviderKind::OpenAi, false, false)),
        OpenAiApi::Responses
    );
    assert_eq!(
        request_api(&request(ProviderKind::OpenAi, false, true)),
        OpenAiApi::Responses
    );
}

#[test]
fn compatible_openai_always_uses_chat_completions() {
    for audio_input in [false, true] {
        for with_audio in [false, true] {
            assert_eq!(
                request_api(&request(
                    ProviderKind::OpenAiCompatible,
                    audio_input,
                    with_audio,
                )),
                OpenAiApi::ChatCompletions
            );
        }
    }
}

#[test]
fn audio_model_keeps_chat_completions_after_tool_loop_messages_are_added() {
    let mut request = request(ProviderKind::OpenAi, true, true);
    assert_eq!(request_api(&request), OpenAiApi::ChatCompletions);
    request.messages.push(Message::assistant("tool call step"));
    request.messages.push(Message::user("tool result step"));
    assert_eq!(request_api(&request), OpenAiApi::ChatCompletions);
}
