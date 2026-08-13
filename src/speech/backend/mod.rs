mod audio_cpp;

use async_trait::async_trait;

use super::{
    error::SpeechError,
    model::{HealthInfo, ModelCatalog, SynthesisRequest, TranscriptionRequest},
};

pub use audio_cpp::AudioCppBackend;

#[async_trait]
pub trait SpeechBackend: Clone + Send + Sync + 'static {
    async fn health(&self) -> Result<HealthInfo, SpeechError>;
    async fn models(&self) -> Result<ModelCatalog, SpeechError>;
    async fn voices(&self, model: &str) -> Result<Vec<String>, SpeechError>;
    async fn synthesize(&self, request: SynthesisRequest) -> Result<Vec<u8>, SpeechError>;
    async fn transcribe(&self, request: TranscriptionRequest) -> Result<String, SpeechError>;
}
