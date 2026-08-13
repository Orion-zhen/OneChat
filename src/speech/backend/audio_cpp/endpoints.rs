use async_trait::async_trait;
use reqwest::{
    header::{ACCEPT, CONTENT_TYPE},
    multipart::{Form, Part},
};
use serde_json::{Value, json};

use super::{
    AudioCppBackend, JSON_SAFE_INTEGER_MAX, WAV_CONTENT_TYPES,
    protocol::{
        insert_optional_number, non_empty, optional_bool, optional_string, required_string,
        transport_error,
    },
};
use crate::speech::{
    backend::SpeechBackend,
    error::SpeechError,
    model::{
        HealthInfo, ModelCatalog, ModelTask, RemoteModel, SynthesisRequest, TranscriptionRequest,
    },
};

#[async_trait]
impl SpeechBackend for AudioCppBackend {
    async fn health(&self) -> Result<HealthInfo, SpeechError> {
        const ENDPOINT: &str = "/health";
        let response = self
            .send_checked(
                ENDPOINT,
                self.request(reqwest::Method::GET, ENDPOINT)
                    .header(ACCEPT, "application/json"),
            )
            .await?;
        let payload = self.json_object(ENDPOINT, response).await?;
        let status = required_string(&payload, "status", ENDPOINT)?;
        let ready = payload
            .get("ready")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| {
                matches!(
                    status.to_ascii_lowercase().as_str(),
                    "ok" | "ready" | "healthy"
                )
            });
        let configured_models = match payload.get("models") {
            Some(Value::Array(models)) => Some(models.len()),
            Some(Value::Number(number)) => number.as_u64().and_then(|value| value.try_into().ok()),
            Some(Value::Null) | None => None,
            Some(_) => {
                return Err(SpeechError::protocol(
                    ENDPOINT,
                    "audio.cpp health response contains an invalid 'models' field",
                ));
            }
        };
        Ok(HealthInfo {
            ready,
            status,
            backend: optional_string(&payload, "backend", ENDPOINT)?,
            configured_models,
        })
    }

    async fn models(&self) -> Result<ModelCatalog, SpeechError> {
        const ENDPOINT: &str = "/v1/models";
        let response = self
            .send_checked(
                ENDPOINT,
                self.request(reqwest::Method::GET, ENDPOINT)
                    .header(ACCEPT, "application/json"),
            )
            .await?;
        let payload = self.json_object(ENDPOINT, response).await?;
        let entries = payload
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                SpeechError::protocol(
                    ENDPOINT,
                    "audio.cpp models response is missing a 'data' array",
                )
            })?;
        let mut catalog = ModelCatalog::default();
        for (index, entry) in entries.iter().enumerate() {
            let entry = entry.as_object().ok_or_else(|| {
                SpeechError::protocol(
                    ENDPOINT,
                    format!("audio.cpp model entry {index} is not an object"),
                )
            })?;
            let task = match required_string(entry, "task", ENDPOINT)?.as_str() {
                "tts" => ModelTask::Tts,
                "asr" => ModelTask::Asr,
                other => ModelTask::Other(other.to_owned()),
            };
            let model = RemoteModel {
                id: required_string(entry, "id", ENDPOINT)?,
                owned_by: optional_string(entry, "owned_by", ENDPOINT)?,
                family: optional_string(entry, "family", ENDPOINT)?,
                task: task.clone(),
                mode: optional_string(entry, "mode", ENDPOINT)?,
                loaded: optional_bool(entry, "loaded", ENDPOINT)?,
                path: optional_string(entry, "path", ENDPOINT)?,
            };
            match task {
                ModelTask::Tts => catalog.tts.push(model),
                ModelTask::Asr => catalog.asr.push(model),
                ModelTask::Other(_) => {}
            }
        }
        Ok(catalog)
    }

    async fn voices(&self, model: &str) -> Result<Vec<String>, SpeechError> {
        const ENDPOINT: &str = "/v1/audio/voices";
        let model = non_empty(model, "a TTS model id is required to list voices")?;
        let response = self
            .send_checked(
                ENDPOINT,
                self.request(reqwest::Method::GET, ENDPOINT)
                    .query(&[("model", model)])
                    .header(ACCEPT, "application/json"),
            )
            .await?;
        let payload = self.json_object(ENDPOINT, response).await?;
        let voices = payload
            .get("voices")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                SpeechError::protocol(
                    ENDPOINT,
                    "audio.cpp voices response is missing a 'voices' array",
                )
            })?;
        voices
            .iter()
            .enumerate()
            .map(|(index, voice)| {
                voice
                    .as_str()
                    .map(str::trim)
                    .filter(|voice| !voice.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        SpeechError::protocol(
                            ENDPOINT,
                            format!("audio.cpp voice entry {index} is not a non-empty string"),
                        )
                    })
            })
            .collect()
    }

    async fn synthesize(&self, request: SynthesisRequest) -> Result<Vec<u8>, SpeechError> {
        const ENDPOINT: &str = "/v1/audio/speech";
        let model = non_empty(
            &request.model,
            "a TTS model id is required for speech generation",
        )?;
        let input = non_empty(&request.input, "speech input text must not be empty")?;
        let mut payload = request.extra_options;
        for reserved in ["model", "input", "voice", "response_format", "seed"] {
            if payload.contains_key(reserved) {
                return Err(SpeechError::configuration(format!(
                    "speech options contain reserved field '{reserved}'"
                )));
            }
        }
        payload.insert("model".into(), Value::String(model.to_owned()));
        payload.insert("input".into(), Value::String(input.to_owned()));
        payload.insert("response_format".into(), Value::String("wav".into()));
        if let Some(voice) = request
            .voice
            .as_deref()
            .map(str::trim)
            .filter(|voice| !voice.is_empty())
        {
            payload.insert("voice".into(), Value::String(voice.into()));
        }
        if let Some(seed) = request.seed {
            payload.insert(
                "seed".into(),
                if seed <= JSON_SAFE_INTEGER_MAX {
                    json!(seed)
                } else {
                    Value::String(seed.to_string())
                },
            );
        }
        insert_optional_number(
            &mut payload,
            "max_tokens",
            request.max_tokens.map(f64::from),
        )?;
        insert_optional_number(&mut payload, "speed", request.speed.map(f64::from))?;
        insert_optional_number(
            &mut payload,
            "temperature",
            request.temperature.map(f64::from),
        )?;
        insert_optional_number(&mut payload, "top_p", request.top_p.map(f64::from))?;

        let response = self
            .send_checked(
                ENDPOINT,
                self.request(reqwest::Method::POST, ENDPOINT)
                    .header(ACCEPT, "audio/wav")
                    .json(&payload),
            )
            .await?;
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if !WAV_CONTENT_TYPES.contains(&content_type.as_str()) {
            return Err(SpeechError::protocol(
                ENDPOINT,
                format!(
                    "audio.cpp speech response has invalid content type {:?}",
                    if content_type.is_empty() {
                        "(missing)"
                    } else {
                        &content_type
                    }
                ),
            ));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| transport_error(ENDPOINT, error))?
            .to_vec();
        if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return Err(SpeechError::protocol(
                ENDPOINT,
                "audio.cpp speech response is not a WAV file",
            ));
        }
        Ok(bytes)
    }

    async fn transcribe(&self, request: TranscriptionRequest) -> Result<String, SpeechError> {
        const ENDPOINT: &str = "/v1/audio/transcriptions";
        let model = non_empty(
            &request.model,
            "an ASR model id is required for transcription",
        )?;
        if request.wav.is_empty() {
            return Err(SpeechError::configuration(
                "a non-empty WAV file is required for transcription",
            ));
        }
        let file = Part::bytes(request.wav)
            .file_name("segment.wav")
            .mime_str("audio/wav")
            .map_err(|error| {
                SpeechError::configuration(format!("invalid WAV MIME type: {error}"))
            })?;
        let mut form = Form::new()
            .text("model", model.to_owned())
            .part("file", file);
        if let Some(language) = request
            .language
            .as_deref()
            .map(str::trim)
            .filter(|language| !language.is_empty())
        {
            form = form.text("language", language.to_owned());
        }
        let response = self
            .send_checked(
                ENDPOINT,
                self.request(reqwest::Method::POST, ENDPOINT)
                    .header(ACCEPT, "application/json")
                    .multipart(form),
            )
            .await?;
        let payload = self.json_object(ENDPOINT, response).await?;
        required_string(&payload, "text", ENDPOINT)
    }
}
