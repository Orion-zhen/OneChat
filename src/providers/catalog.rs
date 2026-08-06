use std::collections::{BTreeMap, HashSet};

use reqwest::RequestBuilder;
use serde_json::Value;

use crate::domain::{GenerationError, GenerationErrorKind, Provider, ProviderKind};

use super::{classify_provider_error, sdk_base_url, sdk_headers, sdk_http_client};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AvailableModel {
    pub id: String,
    pub vision: bool,
}

pub async fn list_models(provider: &Provider) -> Result<Vec<AvailableModel>, GenerationError> {
    let models = match provider.kind {
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
            list_openai_models(provider).await?
        }
        ProviderKind::Anthropic => list_anthropic_models(provider).await?,
        ProviderKind::Gemini => list_gemini_models(provider).await?,
    };
    Ok(sorted_unique(models))
}

async fn list_openai_models(provider: &Provider) -> Result<Vec<AvailableModel>, GenerationError> {
    let url = format!("{}/models", sdk_base_url(provider)?);
    let response = send_json(authenticated_get(provider, &url)?).await?;
    parse_models(&response, "data", ProviderKind::OpenAi)
}

async fn list_anthropic_models(
    provider: &Provider,
) -> Result<Vec<AvailableModel>, GenerationError> {
    let url = format!("{}/v1/models", sdk_base_url(provider)?);
    let mut models = Vec::new();
    let mut after_id = None;
    let mut seen_cursors = HashSet::new();

    loop {
        let mut page_url = parse_url(&url)?;
        page_url.query_pairs_mut().append_pair("limit", "1000");
        if let Some(after_id) = after_id.as_deref() {
            page_url.query_pairs_mut().append_pair("after_id", after_id);
        }
        let response = send_json(authenticated_get(provider, page_url.as_str())?).await?;
        models.extend(parse_models(&response, "data", ProviderKind::Anthropic)?);

        if !response
            .get("has_more")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            break;
        }
        let cursor = response
            .get("last_id")
            .and_then(Value::as_str)
            .filter(|cursor| !cursor.is_empty())
            .ok_or_else(invalid_model_list)?
            .to_string();
        if !seen_cursors.insert(cursor.clone()) {
            return Err(invalid_model_list());
        }
        after_id = Some(cursor);
    }

    Ok(models)
}

async fn list_gemini_models(provider: &Provider) -> Result<Vec<AvailableModel>, GenerationError> {
    let url = format!("{}/v1beta/models", sdk_base_url(provider)?);
    let mut models = Vec::new();
    let mut page_token = None;
    let mut seen_tokens = HashSet::new();

    loop {
        let mut page_url = parse_url(&url)?;
        page_url.query_pairs_mut().append_pair("pageSize", "1000");
        if let Some(page_token) = page_token.as_deref() {
            page_url
                .query_pairs_mut()
                .append_pair("pageToken", page_token);
        }
        let response = send_json(authenticated_get(provider, page_url.as_str())?).await?;
        let items = response
            .get("models")
            .and_then(Value::as_array)
            .ok_or_else(invalid_model_list)?;
        models.extend(
            items
                .iter()
                .filter(|model| supports_gemini_generation(model))
                .filter_map(|model| available_model(model, ProviderKind::Gemini)),
        );

        let Some(token) = response
            .get("nextPageToken")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
        else {
            break;
        };
        if !seen_tokens.insert(token.to_string()) {
            return Err(invalid_model_list());
        }
        page_token = Some(token.to_string());
    }

    Ok(models)
}

fn authenticated_get(provider: &Provider, url: &str) -> Result<RequestBuilder, GenerationError> {
    let client = sdk_http_client(provider)?;
    let mut request = client.get(url);
    if !provider.api_key.is_empty() {
        request = match provider.kind {
            ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
                request.bearer_auth(&provider.api_key)
            }
            ProviderKind::Anthropic => request
                .header("x-api-key", &provider.api_key)
                .header("anthropic-version", "2023-06-01"),
            ProviderKind::Gemini => request.header("x-goog-api-key", &provider.api_key),
        };
    } else if provider.kind == ProviderKind::Anthropic {
        request = request.header("anthropic-version", "2023-06-01");
    }
    Ok(request.headers(sdk_headers(provider)?))
}

fn parse_url(url: &str) -> Result<reqwest::Url, GenerationError> {
    reqwest::Url::parse(url).map_err(|error| {
        GenerationError::new(
            GenerationErrorKind::UnsupportedParameter,
            "Invalid provider endpoint",
        )
        .with_detail(error.to_string())
    })
}

async fn send_json(request: RequestBuilder) -> Result<Value, GenerationError> {
    let response = request.send().await.map_err(GenerationError::network)?;
    let status = response.status();
    let body = response.text().await.map_err(GenerationError::network)?;
    if !status.is_success() {
        return Err(classify_provider_error(status, &body, None));
    }
    serde_json::from_str(&body).map_err(|error| {
        GenerationError::new(
            GenerationErrorKind::Unknown,
            "Provider returned an invalid model list",
        )
        .with_detail(error.to_string())
    })
}

fn parse_models(
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

fn available_model(metadata: &Value, kind: ProviderKind) -> Option<AvailableModel> {
    let id = metadata.get("id").and_then(Value::as_str).or_else(|| {
        (kind == ProviderKind::Gemini)
            .then(|| metadata.get("name").and_then(Value::as_str))
            .flatten()
    })?;
    let id = id.strip_prefix("models/").unwrap_or(id).trim();
    (!id.is_empty()).then(|| AvailableModel {
        id: id.to_string(),
        vision: vision_from_metadata(metadata),
    })
}

fn supports_gemini_generation(metadata: &Value) -> bool {
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

fn normalized_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn sorted_unique(models: Vec<AvailableModel>) -> Vec<AvailableModel> {
    models
        .into_iter()
        .fold(BTreeMap::new(), |mut models, model| {
            models
                .entry(model.id)
                .and_modify(|vision| *vision |= model.vision)
                .or_insert(model.vision);
            models
        })
        .into_iter()
        .map(|(id, vision)| AvailableModel { id, vision })
        .collect()
}

fn invalid_model_list() -> GenerationError {
    GenerationError::new(
        GenerationErrorKind::Unknown,
        "Provider returned an invalid model list",
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::mpsc,
        time::timeout,
    };

    use super::*;

    #[test]
    fn vision_uses_explicit_model_metadata() {
        assert!(vision_from_metadata(&serde_json::json!({
            "architecture": {"input_modalities": ["text", "image"]}
        })));
        assert!(vision_from_metadata(&serde_json::json!({
            "capabilities": {"vision": true}
        })));
        assert!(!vision_from_metadata(&serde_json::json!({
            "id": "gpt-vision-by-name-only",
            "architecture": {"input_modalities": ["text"]}
        })));
    }

    #[tokio::test]
    async fn openai_models_use_configured_auth_and_metadata() {
        let (endpoint, mut requests) = model_server(vec![serde_json::json!({
            "data": [
                {"id": "text-model"},
                {"id": "vision-model", "architecture": {"modality": "text+image->text"}}
            ]
        })])
        .await;
        let mut provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        provider.endpoint = format!("{endpoint}/v1");
        provider.api_key = "secret".into();
        provider.headers.insert("X-Test".into(), "custom".into());

        assert_eq!(
            list_models(&provider).await.unwrap(),
            vec![
                AvailableModel {
                    id: "text-model".into(),
                    vision: false,
                },
                AvailableModel {
                    id: "vision-model".into(),
                    vision: true,
                },
            ]
        );
        let request = receive_request(&mut requests).await;
        assert!(request.starts_with("GET /v1/models HTTP/1.1"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer secret")
        );
        assert!(request.to_ascii_lowercase().contains("x-test: custom"));
    }

    #[tokio::test]
    async fn anthropic_models_follow_cursor_pagination() {
        let (endpoint, mut requests) = model_server(vec![
            serde_json::json!({
                "data": [{"id": "claude-a"}],
                "has_more": true,
                "last_id": "claude-a"
            }),
            serde_json::json!({
                "data": [{"id": "claude-b", "capabilities": ["vision"]}],
                "has_more": false
            }),
        ])
        .await;
        let mut provider = Provider::new("Anthropic", ProviderKind::Anthropic);
        provider.endpoint = format!("{endpoint}/v1");
        provider.api_key = "secret".into();

        assert_eq!(list_models(&provider).await.unwrap().len(), 2);
        let first = receive_request(&mut requests).await;
        let second = receive_request(&mut requests).await;
        assert!(first.starts_with("GET /v1/models?limit=1000 HTTP/1.1"));
        assert!(second.contains("after_id=claude-a"));
        assert!(first.to_ascii_lowercase().contains("x-api-key: secret"));
        assert!(
            first
                .to_ascii_lowercase()
                .contains("anthropic-version: 2023-06-01")
        );
    }

    #[tokio::test]
    async fn gemini_models_follow_page_tokens_and_only_include_generation_models() {
        let (endpoint, mut requests) = model_server(vec![
            serde_json::json!({
                "models": [
                    {"name": "models/gemini-chat", "supportedGenerationMethods": ["generateContent"]},
                    {"name": "models/gemini-embed", "supportedGenerationMethods": ["embedContent"]}
                ],
                "nextPageToken": "next"
            }),
            serde_json::json!({
                "models": [{"name": "models/gemini-vision", "inputModalities": ["TEXT", "IMAGE"]}]
            }),
        ])
        .await;
        let mut provider = Provider::new("Gemini", ProviderKind::Gemini);
        provider.endpoint = format!("{endpoint}/v1beta");
        provider.api_key = "secret".into();

        assert_eq!(
            list_models(&provider).await.unwrap(),
            vec![
                AvailableModel {
                    id: "gemini-chat".into(),
                    vision: false,
                },
                AvailableModel {
                    id: "gemini-vision".into(),
                    vision: true,
                },
            ]
        );
        let first = receive_request(&mut requests).await;
        let second = receive_request(&mut requests).await;
        assert!(first.starts_with("GET /v1beta/models?pageSize=1000 HTTP/1.1"));
        assert!(second.contains("pageToken=next"));
        assert!(
            first
                .to_ascii_lowercase()
                .contains("x-goog-api-key: secret")
        );
    }

    async fn model_server(responses: Vec<Value>) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = mpsc::channel(responses.len());
        tokio::spawn(async move {
            for response in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request = read_request(&mut stream).await;
                sender
                    .send(String::from_utf8_lossy(&request).into_owned())
                    .await
                    .unwrap();
                let body = response.to_string();
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
            }
        });
        (format!("http://{address}"), receiver)
    }

    async fn receive_request(receiver: &mut mpsc::Receiver<String>) -> String {
        timeout(Duration::from_secs(1), receiver.recv())
            .await
            .unwrap()
            .unwrap()
    }

    async fn read_request(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = stream.read(&mut buffer).await.unwrap();
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                return request;
            }
        }
    }
}
