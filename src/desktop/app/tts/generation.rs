use gpui::{Context, Task};

use super::TtsOperationKind;
use crate::{
    desktop::app::OneChat,
    speech::{
        AudioCppBackend, SegmentStatus, SentencexSegmenter, SpeechConfig, SpeechError, SpeechEvent,
        SpeechPipeline, SpeechRun,
    },
};

enum TtsGeneration {
    Generate {
        source: String,
        config: SpeechConfig,
    },
    Regenerate {
        run: SpeechRun,
        segment_index: usize,
    },
    RetryFailed(SpeechRun),
}

impl TtsGeneration {
    fn kind(&self) -> TtsOperationKind {
        match self {
            Self::Generate { .. } => TtsOperationKind::Generate,
            Self::Regenerate { segment_index, .. } => TtsOperationKind::Regenerate(*segment_index),
            Self::RetryFailed(_) => TtsOperationKind::RetryFailed,
        }
    }

    fn config(&self) -> &SpeechConfig {
        match self {
            Self::Generate { config, .. } => config,
            Self::Regenerate { run, .. } | Self::RetryFailed(run) => &run.snapshot.config,
        }
    }
}

impl OneChat {
    pub(crate) fn start_tts_run(&mut self, cx: &mut Context<Self>) {
        self.sync_tts_draft(cx);
        let source = self.tts.controller.source.clone();
        let config = match self.tts.controller.config.clone().normalized() {
            Ok(config) if !source.trim().is_empty() => config,
            Ok(_) => {
                self.tts.controller.error = Some(SpeechError::configuration(
                    "speech input text must not be empty",
                ));
                cx.notify();
                return;
            }
            Err(error) => {
                self.tts.controller.error = Some(error);
                cx.notify();
                return;
            }
        };
        self.tts
            .controller
            .update_config(|current| *current = config.clone());
        self.stop_tts_audio_playback();
        self.tts.view.expanded_segments.clear();
        self.tts.view.technical_segments.clear();
        self.launch_tts_pipeline(TtsGeneration::Generate { source, config }, cx);
    }

    pub(crate) fn regenerate_tts_segment(&mut self, segment_index: usize, cx: &mut Context<Self>) {
        let Some(run) = self.tts.controller.run.clone() else {
            return;
        };
        self.launch_tts_pipeline(TtsGeneration::Regenerate { run, segment_index }, cx);
    }

    pub(crate) fn retry_failed_tts_segments(&mut self, cx: &mut Context<Self>) {
        let Some(run) = self.tts.controller.run.clone() else {
            return;
        };
        self.launch_tts_pipeline(TtsGeneration::RetryFailed(run), cx);
    }

    pub(crate) fn stop_tts_operation(&mut self, cx: &mut Context<Self>) {
        if self.tts.controller.operation.cancel() {
            cx.notify();
        }
    }

    fn launch_tts_pipeline(&mut self, generation: TtsGeneration, cx: &mut Context<Self>) {
        let kind = generation.kind();
        let Some((operation_id, cancellation)) = self.tts.controller.operation.start(kind) else {
            return;
        };
        let config = generation.config();
        let backend = match AudioCppBackend::new(
            &config.endpoint,
            config.bearer_token.as_deref(),
            config.request_timeout,
        ) {
            Ok(backend) => backend,
            Err(error) => {
                self.tts.controller.operation.finish(operation_id);
                self.tts.controller.error = Some(error);
                cx.notify();
                return;
            }
        };
        let pipeline = SpeechPipeline::new(backend, SentencexSegmenter::default());
        let (sender, receiver) = async_channel::bounded(32);
        let task = self.services.runtime.spawn(async move {
            match generation {
                TtsGeneration::Generate { source, config } => {
                    pipeline.run(source, config, &sender, cancellation).await
                }
                TtsGeneration::Regenerate { run, segment_index } => {
                    pipeline
                        .regenerate_segment(&run, segment_index, &sender, cancellation)
                        .await
                }
                TtsGeneration::RetryFailed(run) => {
                    pipeline
                        .retry_failed_once(&run, &sender, cancellation)
                        .await
                }
            }
        });

        let previous = std::mem::replace(&mut self.tts.event_task, Task::ready(()));
        self.tts.event_task = cx.spawn(async move |this, cx| {
            previous.await;
            while let Ok(event) = receiver.recv().await {
                let _ = this.update(cx, |this, cx| {
                    if !this.tts.controller.operation.is_current(operation_id) {
                        return;
                    }
                    this.apply_tts_progress(event, kind);
                    cx.notify();
                });
            }
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if !this.tts.controller.operation.is_current(operation_id) {
                    return;
                }
                match result {
                    Ok(result) => this.finish_tts_generation(result, kind),
                    Err(error) => this.data.error = Some(format!("Speech task failed: {error}")),
                }
                this.tts.controller.operation.finish(operation_id);
                cx.notify();
            });
        });
        cx.notify();
    }

    fn apply_tts_progress(&mut self, event: SpeechEvent, kind: TtsOperationKind) {
        if let SpeechEvent::SegmentFinished { result } = &event {
            match result.status {
                SegmentStatus::Failed => {
                    self.tts.view.expanded_segments.insert(result.segment.index);
                }
                SegmentStatus::Ready
                    if matches!(
                        kind,
                        TtsOperationKind::Regenerate(_) | TtsOperationKind::RetryFailed
                    ) =>
                {
                    self.tts
                        .view
                        .expanded_segments
                        .remove(&result.segment.index);
                    self.tts
                        .view
                        .technical_segments
                        .remove(&result.segment.index);
                }
                _ => {}
            }
        }
        self.tts.controller.apply_speech_event(event);
    }

    fn finish_tts_generation(
        &mut self,
        result: Result<SpeechRun, SpeechError>,
        kind: TtsOperationKind,
    ) {
        if let Ok(run) = &result {
            self.tts.view.expanded_segments.extend(
                run.segments
                    .iter()
                    .filter(|result| result.status == SegmentStatus::Failed)
                    .map(|result| result.segment.index),
            );
            self.stop_tts_audio_playback();
            self.tts.controller.bump_audio_revision();
            if matches!(kind, TtsOperationKind::Regenerate(_)) {
                self.tts.completion_notice =
                    Some("Segment regenerated and combined audio updated.".into());
            }
        }
        self.tts.controller.finish_speech(result);
    }
}
