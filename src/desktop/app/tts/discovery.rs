use std::time::Duration;

use gpui::Context;

use super::TtsOperationKind;
use crate::{
    desktop::app::OneChat,
    speech::{AudioCppBackend, HealthInfo, ModelCatalog, SpeechBackend, SpeechError},
};

const DISCOVERY_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
struct DiscoveryResult {
    health: HealthInfo,
    catalog: ModelCatalog,
    voices: Vec<String>,
    model: Option<String>,
    voice: Option<String>,
}

impl OneChat {
    pub(crate) fn test_tts_connection(&mut self, cx: &mut Context<Self>) {
        let Some((operation_id, cancellation)) = self
            .tts
            .controller
            .operation
            .start(TtsOperationKind::Discovery)
        else {
            return;
        };
        self.sync_tts_connection_draft(cx);
        self.tts.controller.begin_discovery();
        let config = self.tts.controller.config.clone();
        self.spawn_tokio(
            async move {
                let backend = AudioCppBackend::new(
                    &config.endpoint,
                    config.bearer_token.as_deref(),
                    config.request_timeout.min(DISCOVERY_REQUEST_TIMEOUT),
                )?;
                cancellable(cancellation, backend.health()).await
            },
            cx,
            move |this, result, cx| {
                if !this.tts.controller.operation.finish(operation_id) {
                    return;
                }
                match result {
                    Ok(Ok(health)) => {
                        this.tts.controller.discovery.loading = false;
                        this.tts.controller.discovery.health = Some(health);
                        this.tts.controller.discovery.error = None;
                        this.tts.controller.error = None;
                    }
                    Ok(Err(error)) => this.tts.controller.fail_discovery(error),
                    Err(_) => this.tts.controller.fail_discovery(SpeechError::cancelled()),
                }
                cx.notify();
            },
        );
        cx.notify();
    }

    pub(crate) fn refresh_tts_discovery(&mut self, cx: &mut Context<Self>) {
        let Some((operation_id, cancellation)) = self
            .tts
            .controller
            .operation
            .start(TtsOperationKind::Discovery)
        else {
            return;
        };
        self.sync_tts_connection_draft(cx);
        self.tts.controller.begin_discovery();
        let config = self.tts.controller.config.clone();
        self.spawn_tokio(
            async move {
                let backend = AudioCppBackend::new(
                    &config.endpoint,
                    config.bearer_token.as_deref(),
                    config.request_timeout.min(DISCOVERY_REQUEST_TIMEOUT),
                )?;
                let health = cancellable(cancellation.clone(), backend.health()).await?;
                let catalog = cancellable(cancellation.clone(), backend.models()).await?;
                let model = catalog
                    .tts
                    .iter()
                    .any(|model| model.id == config.generation.model)
                    .then(|| config.generation.model.clone())
                    .or_else(|| catalog.tts.first().map(|model| model.id.clone()));
                let voices = match &model {
                    Some(model) => cancellable(cancellation, backend.voices(model)).await?,
                    None => Vec::new(),
                };
                let voice = config
                    .generation
                    .voice
                    .filter(|voice| voices.contains(voice))
                    .or_else(|| voices.first().cloned());
                Ok::<_, SpeechError>(DiscoveryResult {
                    health,
                    catalog,
                    voices,
                    model,
                    voice,
                })
            },
            cx,
            move |this, result, cx| {
                if !this.tts.controller.operation.finish(operation_id) {
                    return;
                }
                match result {
                    Ok(Ok(discovery)) => {
                        this.tts.controller.update_config(|config| {
                            config.generation.model = discovery.model.unwrap_or_default();
                            config.generation.voice = discovery.voice;
                        });
                        this.tts.controller.apply_discovery(
                            discovery.health,
                            discovery.catalog,
                            discovery.voices,
                        );
                        this.tts.view.connection_popover_open = false;
                    }
                    Ok(Err(error)) => this.tts.controller.fail_discovery(error),
                    Err(_) => this.tts.controller.fail_discovery(SpeechError::cancelled()),
                }
                cx.notify();
            },
        );
        cx.notify();
    }

    pub(crate) fn select_tts_model(&mut self, model: Option<String>, cx: &mut Context<Self>) {
        self.tts.controller.update_config(|config| {
            config.generation.model = model.unwrap_or_default();
            config.generation.voice = None;
        });
        self.tts.controller.discovery.voices.clear();
        self.refresh_tts_discovery(cx);
    }

    pub(crate) fn select_tts_voice(&mut self, voice: Option<String>, cx: &mut Context<Self>) {
        self.tts
            .controller
            .update_config(|config| config.generation.voice = voice);
        cx.notify();
    }

    pub(super) fn sync_tts_connection_draft(&mut self, cx: &mut Context<Self>) {
        let endpoint = self
            .tts
            .controls
            .connection
            .endpoint
            .read(cx)
            .value()
            .to_string();
        let token = self
            .tts
            .controls
            .connection
            .token
            .read(cx)
            .value()
            .trim()
            .to_string();
        let token = (!token.is_empty()).then_some(token);
        let connection_changed = self.tts.controller.config.endpoint != endpoint
            || self.tts.controller.config.bearer_token != token;
        self.tts.controller.update_config(|config| {
            config.endpoint = endpoint;
            config.bearer_token = token;
            if connection_changed {
                config.generation.model.clear();
                config.generation.voice = None;
            }
        });
        if connection_changed {
            self.tts.controller.discovery = Default::default();
        }
    }
}

async fn cancellable<T>(
    cancellation: tokio_util::sync::CancellationToken,
    future: impl Future<Output = Result<T, SpeechError>>,
) -> Result<T, SpeechError> {
    tokio::select! {
        _ = cancellation.cancelled() => Err(SpeechError::cancelled()),
        result = future => result,
    }
}
