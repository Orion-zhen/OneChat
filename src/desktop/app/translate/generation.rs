use std::time::Instant;

use gpui::Context;

use super::{
    content::{prompts_include_text, render_prompt},
    languages::{resolved_source_language, same_language},
};
use crate::{
    application::{
        context_usage::estimate_input_tokens,
        generation::{GenerationStream, UI_FLUSH_INTERVAL},
    },
    desktop::app::{OneChat, Page},
    domain::{
        AssistantResponse, GenerationConfig, GenerationRequest, Message, MessageStatus, Model,
        RequestInfo, new_id,
    },
    providers,
};

const TRANSLATION_CONVERSATION_ID: &str = "translation-playground";

impl OneChat {
    pub(crate) fn translation_model(&self) -> Option<&Model> {
        self.translation
            .model_id
            .as_deref()
            .and_then(|model_id| {
                self.data
                    .snapshot
                    .models
                    .iter()
                    .find(|model| model.id == model_id)
            })
            .or_else(|| self.primary_model())
    }

    pub(crate) fn start_translation(&mut self, cx: &mut Context<Self>) {
        if self.translation.is_generating() {
            return;
        }
        let source = self.translation.source.trim().to_string();
        if source.is_empty() {
            self.translation.error = Some("Enter text to translate.".into());
            cx.notify();
            return;
        }
        let source_language = resolved_source_language(&self.translation.source_language, &source);
        if same_language(&source_language, &self.translation.target_language) {
            self.translation.error = Some("Source and target languages must be different.".into());
            cx.notify();
            return;
        }
        if !prompts_include_text(
            &self.translation.system_prompt,
            &self.translation.user_prompt,
        ) {
            self.translation.error = Some("A prompt must include {{text}}.".into());
            cx.notify();
            return;
        }

        let Some(model) = self.translation_model().cloned() else {
            self.translation.error = Some("Choose a model before translating.".into());
            cx.notify();
            return;
        };
        if let Err(reason) = self.model_availability(&model) {
            self.translation.error = Some(format!("Model is unavailable: {reason}."));
            cx.notify();
            return;
        }
        let Some(provider) = self.provider_for_model(&model).cloned() else {
            self.translation.error = Some("The selected model has no provider.".into());
            cx.notify();
            return;
        };

        let system_prompt = render_prompt(
            &self.translation.system_prompt,
            &source,
            &source_language,
            &self.translation.target_language,
        );
        let user_prompt = render_prompt(
            &self.translation.user_prompt,
            &source,
            &source_language,
            &self.translation.target_language,
        );
        let messages = vec![Message::user(user_prompt)];
        let mut config = GenerationConfig::default();
        if model.capabilities.temperature {
            config.temperature = Some(0.2);
        }
        config.reasoning_preset = self.translation.reasoning_preset.clone();
        let (config, _) = config.filtered_for(&model.capabilities);
        let provider_request = GenerationRequest {
            provider: provider.clone(),
            model: model.clone(),
            system_prompt: system_prompt.clone(),
            config,
            messages: messages.clone(),
            audio_duration_ms: 0,
            tools: Vec::new(),
        };

        let mut response = AssistantResponse::new(&model, &provider);
        response.status = MessageStatus::Streaming;
        let turn_id = new_id("translation");
        let mut request =
            RequestInfo::new(TRANSLATION_CONVERSATION_ID, turn_id, response.id.clone());
        request.provider_id = Some(provider.id.clone());
        request.model_id = Some(model.id.clone());
        request.usage.input_tokens = Some(estimate_input_tokens(&system_prompt, &messages, 0));
        request.usage.estimated = true;
        response.request_id = Some(request.id.clone());

        let (operation_id, cancellation) = self
            .translation
            .output
            .begin(response.clone(), request.clone());
        self.translation.error = None;
        cx.notify();

        let (sender, receiver) = async_channel::bounded(256);
        self.services
            .runtime
            .spawn(providers::generate(provider_request, sender, cancellation));

        cx.spawn(async move |this, cx| {
            let started = Instant::now();
            let mut stream = GenerationStream::new(receiver, response, request);
            while !stream.snapshot.terminal {
                cx.background_executor().timer(UI_FLUSH_INTERVAL).await;
                if !stream.drain(started.elapsed()) {
                    continue;
                }
                let snapshot = stream.snapshot.clone();
                let _ = this.update(cx, |this, cx| {
                    if this.translation.output.apply(operation_id, snapshot) {
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    pub(crate) fn stop_translation(&mut self, cx: &mut Context<Self>) {
        self.translation.output.stop();
        cx.notify();
    }

    pub(crate) fn run_translation_action(&mut self, cx: &mut Context<Self>) {
        if self.navigation.page == Page::Translate {
            self.start_translation(cx);
        }
    }
}
