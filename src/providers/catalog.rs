use std::collections::{BTreeMap, HashSet};

use reqwest::RequestBuilder;
use serde_json::Value;

use crate::domain::{GenerationError, GenerationErrorKind, Provider, ProviderKind};

use super::{classify_provider_error, sdk_base_url, sdk_headers, sdk_http_client};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AvailableModel {
    pub id: String,
    pub tools: bool,
    pub vision: bool,
    pub audio_input: bool,
    pub context_window_tokens: Option<u32>,
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

mod fetch;
mod metadata;

use fetch::{list_anthropic_models, list_gemini_models, list_openai_models};
use metadata::sorted_unique;
