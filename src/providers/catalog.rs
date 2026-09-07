use std::collections::{BTreeMap, HashSet};

use reqwest::RequestBuilder;
use serde_json::Value;

use crate::domain::{
    GenerationError, GenerationErrorKind, ModelIdReasoningConfig, ModelIdReasoningPreset, Provider,
    ProviderKind, ReasoningLevel, split_reasoning_model_id,
};

use super::{classify_provider_error, sdk_base_url, sdk_headers, sdk_http_client};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AvailableModel {
    pub id: String,
    pub reasoning: Option<ModelIdReasoningConfig>,
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
    Ok(model_id::group_reasoning_models(sorted_unique(models)))
}

mod fetch;
mod metadata;
mod model_id;

use fetch::{list_anthropic_models, list_gemini_models, list_openai_models};
use metadata::sorted_unique;
