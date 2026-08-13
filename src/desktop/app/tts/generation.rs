use async_channel::Sender;
use gpui::{Context, Task};

use super::TtsOperationKind;
use crate::{
    desktop::app::OneChat,
    speech::{
        AudioCppBackend, SentencexSegmenter, SpeechError, SpeechEvent, SpeechPipeline, SpeechRun,
    },
};

enum TtsMessage {
    Event(SpeechEvent),
    Finished(Box<Result<SpeechRun, SpeechError>>),
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
        self.launch_tts_pipeline(
            TtsOperationKind::Generate,
            cx,
            move |pipeline, events, token| {
                Box::pin(async move { pipeline.run(source, config, &events, token).await })
            },
        );
    }

    pub(crate) fn regenerate_tts_segment(&mut self, segment_index: usize, cx: &mut Context<Self>) {
        let Some(run) = self.tts.controller.run.clone() else {
            return;
        };
        self.launch_tts_pipeline(
            TtsOperationKind::Regenerate(segment_index),
            cx,
            move |pipeline, events, token| {
                Box::pin(async move {
                    pipeline
                        .regenerate_segment(&run, segment_index, &events, token)
                        .await
                })
            },
        );
    }

    pub(crate) fn retry_failed_tts_segments(&mut self, cx: &mut Context<Self>) {
        let Some(run) = self.tts.controller.run.clone() else {
            return;
        };
        self.launch_tts_pipeline(
            TtsOperationKind::RetryFailed,
            cx,
            move |pipeline, events, token| {
                Box::pin(async move { pipeline.retry_failed_once(&run, &events, token).await })
            },
        );
    }

    pub(crate) fn stop_tts_operation(&mut self, cx: &mut Context<Self>) {
        if self.tts.controller.operation.cancel() {
            cx.notify();
        }
    }

    fn launch_tts_pipeline<F>(&mut self, kind: TtsOperationKind, cx: &mut Context<Self>, run: F)
    where
        F: FnOnce(
                SpeechPipeline<AudioCppBackend, SentencexSegmenter>,
                Sender<SpeechEvent>,
                tokio_util::sync::CancellationToken,
            )
                -> std::pin::Pin<Box<dyn Future<Output = Result<SpeechRun, SpeechError>> + Send>>
            + Send
            + 'static,
    {
        let Some((operation_id, cancellation)) = self.tts.controller.operation.start(kind) else {
            return;
        };
        let snapshot_config = match kind {
            TtsOperationKind::Generate => self.tts.controller.config.clone(),
            TtsOperationKind::Regenerate(_) | TtsOperationKind::RetryFailed => self
                .tts
                .controller
                .run
                .as_ref()
                .map(|run| run.snapshot.config.clone())
                .unwrap_or_else(|| self.tts.controller.config.clone()),
            TtsOperationKind::Discovery => unreachable!("discovery uses its own task"),
        };
        let backend = match AudioCppBackend::new(
            &snapshot_config.endpoint,
            snapshot_config.bearer_token.as_deref(),
            snapshot_config.request_timeout,
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
        let (event_sender, event_receiver) = async_channel::bounded(32);
        let (message_sender, message_receiver) = async_channel::bounded(32);
        self.services.runtime.spawn(async move {
            let event_message_sender = message_sender.clone();
            let forward = tokio::spawn(async move {
                while let Ok(event) = event_receiver.recv().await {
                    if event_message_sender
                        .send(TtsMessage::Event(event))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            });
            let result = run(pipeline, event_sender, cancellation).await;
            let _ = forward.await;
            let _ = message_sender
                .send(TtsMessage::Finished(Box::new(result)))
                .await;
        });

        let previous = std::mem::replace(&mut self.tts.event_task, Task::ready(()));
        self.tts.event_task = cx.spawn(async move |this, cx| {
            previous.await;
            while let Ok(message) = message_receiver.recv().await {
                let finished = matches!(message, TtsMessage::Finished(_));
                let _ = this.update(cx, |this, cx| {
                    if !this.tts.controller.operation.is_current(operation_id) {
                        return;
                    }
                    match message {
                        TtsMessage::Event(event) => {
                            if let SpeechEvent::SegmentFinished { result } = &event {
                                match result.status {
                                    crate::speech::SegmentStatus::Failed => {
                                        this.tts
                                            .view
                                            .expanded_segments
                                            .insert(result.segment.index);
                                    }
                                    crate::speech::SegmentStatus::Ready
                                        if this.tts.controller.operation.active().is_some_and(
                                            |active| {
                                                matches!(
                                                    active.kind,
                                                    TtsOperationKind::Regenerate(_)
                                                        | TtsOperationKind::RetryFailed
                                                )
                                            },
                                        ) =>
                                    {
                                        this.tts
                                            .view
                                            .expanded_segments
                                            .remove(&result.segment.index);
                                        this.tts
                                            .view
                                            .technical_segments
                                            .remove(&result.segment.index);
                                    }
                                    _ => {}
                                }
                            }
                            if let SpeechEvent::RunFinished { run } = &event {
                                this.tts.view.expanded_segments.extend(
                                    run.segments
                                        .iter()
                                        .filter(|result| {
                                            result.status == crate::speech::SegmentStatus::Failed
                                        })
                                        .map(|result| result.segment.index),
                                );
                                this.stop_tts_audio_playback();
                                this.tts.controller.bump_audio_revision();
                            }
                            this.tts.controller.apply_speech_event(event);
                        }
                        TtsMessage::Finished(result) => {
                            let kind = this
                                .tts
                                .controller
                                .operation
                                .active()
                                .map(|active| active.kind);
                            let succeeded = result.is_ok();
                            this.tts.controller.finish_speech(*result);
                            this.tts.controller.operation.finish(operation_id);
                            if succeeded && matches!(kind, Some(TtsOperationKind::Regenerate(_))) {
                                this.tts.completion_notice =
                                    Some("Segment regenerated and combined audio updated.".into());
                            }
                        }
                    }
                    cx.notify();
                });
                if finished {
                    break;
                }
            }
        });
        cx.notify();
    }
}
