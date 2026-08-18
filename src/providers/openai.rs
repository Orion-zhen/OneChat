use async_channel::Sender;
use rig_core::{
    client::{CompletionClient, VerifyClient},
    completion::Message,
    providers::openai as rig_openai,
};
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{
        GenerationError, GenerationErrorKind, GenerationEvent, GenerationRequest, Provider,
        ProviderKind,
    },
    providers::{
        insert_optional, merged_additional_parameters, remove_keys, sdk_base_url, sdk_headers,
        sdk_http_client, sdk_request, sdk_verify_error, stream_model,
    },
};

type OpenAiClient = rig_openai::Client<reqwest::Client>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OpenAiApi {
    Responses,
    ChatCompletions,
}

fn request_api(request: &GenerationRequest) -> OpenAiApi {
    match request.provider.kind {
        ProviderKind::OpenAi if request.model.capabilities.audio_input => {
            OpenAiApi::ChatCompletions
        }
        ProviderKind::OpenAi => OpenAiApi::Responses,
        ProviderKind::OpenAiCompatible => OpenAiApi::ChatCompletions,
        _ => unreachable!("request_api called for a non-OpenAI provider"),
    }
}

pub async fn test_connection(provider: &Provider) -> Result<(), GenerationError> {
    build_client(provider)?
        .verify()
        .await
        .map_err(sdk_verify_error)
}

pub async fn stream(
    request: GenerationRequest,
    events: &Sender<GenerationEvent>,
    cancellation: CancellationToken,
) -> Result<Message, GenerationError> {
    if cancellation.is_cancelled() {
        return Err(GenerationError::cancelled());
    }

    let client = build_client(&request.provider)?;
    let model_id = request.model.remote_id.clone();
    let sdk_request = sdk_request(&request, additional_parameters(&request)?)?;
    match request_api(&request) {
        OpenAiApi::Responses => {
            stream_model(
                client.completion_model(model_id),
                sdk_request,
                events,
                cancellation,
                false,
            )
            .await
        }
        OpenAiApi::ChatCompletions => {
            stream_model(
                client.completions_api().completion_model(model_id),
                sdk_request,
                events,
                cancellation,
                false,
            )
            .await
        }
    }
}

fn additional_parameters(
    request: &GenerationRequest,
) -> Result<Map<String, Value>, GenerationError> {
    let capabilities = &request.model.capabilities;
    let config = &request.config;
    let mut parameters = merged_additional_parameters(request)?;
    remove_keys(
        &mut parameters,
        &[
            "model",
            "messages",
            "stream",
            "temperature",
            "top_p",
            "top_k",
            "max_tokens",
            "max_completion_tokens",
            "frequency_penalty",
            "presence_penalty",
            "seed",
            "stop",
            "tools",
            "tool_choice",
            "parallel_tool_calls",
            "modalities",
            "audio",
        ],
    );
    insert_optional(
        &mut parameters,
        "top_p",
        capabilities.top_p.then_some(config.top_p).flatten(),
    );
    insert_optional(
        &mut parameters,
        "top_k",
        capabilities.top_k.then_some(config.top_k).flatten(),
    );
    insert_optional(
        &mut parameters,
        "frequency_penalty",
        capabilities
            .frequency_penalty
            .then_some(config.frequency_penalty)
            .flatten(),
    );
    insert_optional(
        &mut parameters,
        "presence_penalty",
        capabilities
            .presence_penalty
            .then_some(config.presence_penalty)
            .flatten(),
    );
    insert_optional(
        &mut parameters,
        "seed",
        capabilities.seed.then_some(config.seed).flatten(),
    );
    if capabilities.stop_sequences && !config.stop_sequences.is_empty() {
        parameters.insert("stop".into(), json!(config.stop_sequences));
    }
    Ok(parameters)
}

fn build_client(provider: &Provider) -> Result<OpenAiClient, GenerationError> {
    rig_openai::Client::builder()
        .api_key(provider.api_key.clone())
        .base_url(sdk_base_url(provider)?)
        .http_headers(sdk_headers(provider)?)
        .http_client(sdk_http_client(provider)?)
        .build()
        .map_err(|error| {
            GenerationError::new(
                GenerationErrorKind::UnsupportedParameter,
                "Invalid provider configuration",
            )
            .with_detail(error.to_string())
        })
}

#[cfg(test)]
mod tests {
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
}
