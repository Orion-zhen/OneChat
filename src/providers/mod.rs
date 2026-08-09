mod catalog;
mod common;
mod error;

mod anthropic;
mod gemini;
mod openai;

use async_channel::Sender;
use tokio_util::sync::CancellationToken;

use crate::domain::{
    GenerationError, GenerationErrorKind, GenerationEvent, GenerationRequest, Message, Provider,
    ProviderKind, message_tool_calls,
};

pub use catalog::{AvailableModel, list_models};
pub(crate) use common::{
    consume_stream, insert_optional, merged_additional_parameters, remove_keys, sdk_base_url,
    sdk_headers, sdk_http_client, sdk_request,
};
pub(crate) use error::{classify_provider_error, sdk_completion_error, sdk_verify_error};

pub async fn test_connection(provider: &Provider) -> Result<(), GenerationError> {
    match provider.kind {
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
            openai::test_connection(provider).await
        }
        ProviderKind::Anthropic => anthropic::test_connection(provider).await,
        ProviderKind::Gemini => gemini::test_connection(provider).await,
    }
}

pub async fn stream_step(
    request: GenerationRequest,
    events: &Sender<GenerationEvent>,
    cancellation: CancellationToken,
) -> Result<Message, GenerationError> {
    match request.provider.kind {
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible => {
            openai::stream(request, events, cancellation).await
        }
        ProviderKind::Anthropic => anthropic::stream(request, events, cancellation).await,
        ProviderKind::Gemini => gemini::stream(request, events, cancellation).await,
    }
}

pub async fn generate(
    request: GenerationRequest,
    events: Sender<GenerationEvent>,
    cancellation: CancellationToken,
) {
    match stream_step(request, &events, cancellation).await {
        Ok(message) if message_tool_calls(&message).is_empty() => {
            let _ = events.send(GenerationEvent::Completed).await;
        }
        Ok(_) => {
            let _ = events
                .send(GenerationEvent::Failed(GenerationError::new(
                    GenerationErrorKind::Unknown,
                    "Unexpected tool call",
                )))
                .await;
        }
        Err(error) => {
            let _ = events.send(GenerationEvent::Failed(error)).await;
        }
    }
}
