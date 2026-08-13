use reqwest::StatusCode;
use serde_json::{Map, Value};

use crate::speech::error::SpeechError;

pub(super) fn non_empty<'a>(value: &'a str, message: &str) -> Result<&'a str, SpeechError> {
    let value = value.trim();
    if value.is_empty() {
        Err(SpeechError::configuration(message))
    } else {
        Ok(value)
    }
}

pub(super) fn required_string(
    object: &Map<String, Value>,
    field: &str,
    endpoint: &str,
) -> Result<String, SpeechError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            SpeechError::protocol(
                endpoint,
                format!("audio.cpp response is missing a non-empty string '{field}' field"),
            )
        })
}

pub(super) fn optional_string(
    object: &Map<String, Value>,
    field: &str,
    endpoint: &str,
) -> Result<Option<String>, SpeechError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(SpeechError::protocol(
            endpoint,
            format!("audio.cpp response contains an invalid '{field}' field"),
        )),
    }
}

pub(super) fn optional_bool(
    object: &Map<String, Value>,
    field: &str,
    endpoint: &str,
) -> Result<Option<bool>, SpeechError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(SpeechError::protocol(
            endpoint,
            format!("audio.cpp response contains an invalid '{field}' field"),
        )),
    }
}

pub(super) fn insert_optional_number(
    payload: &mut Map<String, Value>,
    field: &str,
    value: Option<f64>,
) -> Result<(), SpeechError> {
    if let Some(value) = value {
        if payload.contains_key(field) {
            return Err(SpeechError::configuration(format!(
                "speech options contain duplicate field '{field}'"
            )));
        }
        let number = serde_json::Number::from_f64(value).ok_or_else(|| {
            SpeechError::configuration(format!("speech option '{field}' must be finite"))
        })?;
        payload.insert(field.into(), Value::Number(number));
    }
    Ok(())
}

pub(super) fn transport_error(endpoint: &str, error: reqwest::Error) -> SpeechError {
    let message = if error.is_timeout() {
        format!("audio.cpp request timed out: {endpoint}")
    } else {
        format!("could not connect to audio.cpp: {endpoint}")
    };
    SpeechError::transport(endpoint, message, error.to_string())
}

pub(super) fn service_error_message(bytes: &[u8], status: StatusCode) -> String {
    if let Ok(Value::Object(payload)) = serde_json::from_slice::<Value>(bytes) {
        if let Some(Value::Object(error)) = payload.get("error") {
            for field in ["message", "detail", "type", "code"] {
                if let Some(message) = error
                    .get(field)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|message| !message.is_empty())
                {
                    return message.to_owned();
                }
            }
        }
        for field in ["message", "detail"] {
            if let Some(message) = payload
                .get(field)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|message| !message.is_empty())
            {
                return message.to_owned();
            }
        }
    }
    let body = String::from_utf8_lossy(bytes).trim().to_owned();
    if body.is_empty() {
        status
            .canonical_reason()
            .unwrap_or("unknown server error")
            .to_owned()
    } else {
        body
    }
}
