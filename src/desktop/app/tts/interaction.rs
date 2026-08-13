use gpui::Context;

use crate::{desktop::app::OneChat, speech::SpeechConfig};

impl OneChat {
    pub(crate) fn set_tts_connection_popover_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.tts.view.connection_popover_open = open;
        cx.notify();
    }

    pub(crate) fn set_tts_inspector_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.tts.view.inspector_open = open;
        self.tts.inspector_motion.set_open(open, true);
        cx.notify();
    }

    pub(crate) fn toggle_tts_segment(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.tts.view.expanded_segments.remove(&index) {
            self.tts.view.expanded_segments.insert(index);
        }
        cx.notify();
    }

    pub(crate) fn toggle_tts_technical_details(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.tts.view.technical_segments.remove(&index) {
            self.tts.view.technical_segments.insert(index);
        }
        cx.notify();
    }

    pub(crate) fn toggle_tts_audio_thresholds(&mut self, cx: &mut Context<Self>) {
        self.tts.view.audio_thresholds_expanded = !self.tts.view.audio_thresholds_expanded;
        cx.notify();
    }

    pub(crate) fn toggle_tts_transcript_details(&mut self, cx: &mut Context<Self>) {
        self.tts.view.transcript_details_expanded = !self.tts.view.transcript_details_expanded;
        cx.notify();
    }

    pub(crate) fn toggle_tts_transcript_validation(&mut self, cx: &mut Context<Self>) {
        let fallback_model = self
            .tts
            .controller
            .discovery
            .catalog
            .asr
            .first()
            .map(|model| model.id.clone());
        self.tts.controller.update_config(|config| {
            config.transcript_validation.enabled = !config.transcript_validation.enabled;
            if config.transcript_validation.enabled && config.transcript_validation.model.is_none()
            {
                config.transcript_validation.model = fallback_model;
            }
        });
        cx.notify();
    }

    pub(crate) fn reset_tts_tuning(&mut self, cx: &mut Context<Self>) {
        let defaults = SpeechConfig::default();
        self.tts.controller.update_config(|config| {
            let model = std::mem::take(&mut config.generation.model);
            let voice = config.generation.voice.take();
            config.request_timeout = defaults.request_timeout;
            config.generation = defaults.generation;
            config.generation.model = model;
            config.generation.voice = voice;
            config.segmentation = defaults.segmentation;
            config.audio_validation = defaults.audio_validation;
            config.transcript_validation = defaults.transcript_validation;
            config.merge = defaults.merge;
            config.transport_retries = defaults.transport_retries;
            config.transport_backoff = defaults.transport_backoff;
            config.quality_retries = defaults.quality_retries;
        });
        cx.notify();
    }

    pub(super) fn sync_tts_tuning_draft(&mut self, cx: &mut Context<Self>) {
        let tuning = &self.tts.controls.tuning;
        let max_chars = tuning.max_chars.read(cx).value().start() as usize;
        let target_chars =
            (tuning.target_chars.read(cx).value().start() as usize).clamp(1, max_chars);
        let min_chars = (tuning.min_chars.read(cx).value().start() as usize).clamp(1, target_chars);
        let spread = tuning.spread.read(cx).value().start() as usize;
        let min_duration = tuning.min_duration.read(cx).value().start();
        let min_rms = tuning.min_rms.read(cx).value().start();
        let min_active_ratio = tuning.min_active_ratio.read(cx).value().start();
        let noise_flatness = tuning.noise_flatness.read(cx).value().start();
        let noise_zcr = tuning.noise_zcr.read(cx).value().start();
        let noise_active_ratio = tuning.noise_active_ratio.read(cx).value().start();
        let trim_max_silence = tuning.trim_max_silence.read(cx).value().start();
        let trim_keep_silence = tuning
            .trim_keep_silence
            .read(cx)
            .value()
            .start()
            .min(trim_max_silence);
        let merge_silence = tuning.merge_silence.read(cx).value().start();
        let similarity = tuning.similarity.read(cx).value().start();
        self.tts.controller.update_config(|config| {
            config.segmentation.min_chars = min_chars;
            config.segmentation.target_chars = target_chars;
            config.segmentation.max_chars = max_chars;
            config.segmentation.spread = spread;
            config.audio_validation.min_duration_sec = min_duration;
            config.audio_validation.min_rms = min_rms;
            config.audio_validation.min_active_ratio = min_active_ratio;
            config.audio_validation.noise_flatness = noise_flatness;
            config.audio_validation.noise_zcr = noise_zcr;
            config.audio_validation.noise_active_ratio = noise_active_ratio;
            config.audio_validation.trim_max_edge_silence_sec = trim_max_silence;
            config.audio_validation.trim_keep_edge_silence_sec = trim_keep_silence;
            config.merge.min_silence_sec = merge_silence;
            config.transcript_validation.similarity_threshold = similarity;
        });
    }

    pub(super) fn sync_tts_draft(&mut self, cx: &mut Context<Self>) {
        self.sync_tts_connection_draft(cx);
        self.tts
            .controller
            .set_source(self.tts.controls.source.read(cx).value().to_string());
        self.sync_tts_tuning_draft(cx);
    }
}
