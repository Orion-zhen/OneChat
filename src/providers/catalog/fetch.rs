use super::metadata::{
    available_model, invalid_model_list, parse_models, supports_gemini_generation,
};
use super::*;

pub(super) async fn list_openai_models(
    provider: &Provider,
) -> Result<Vec<AvailableModel>, GenerationError> {
    let url = format!("{}/models", sdk_base_url(provider)?);
    let response = send_json(authenticated_get(provider, &url)?).await?;
    parse_models(&response, "data", ProviderKind::OpenAi)
}

pub(super) async fn list_anthropic_models(
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

pub(super) async fn list_gemini_models(
    provider: &Provider,
) -> Result<Vec<AvailableModel>, GenerationError> {
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
