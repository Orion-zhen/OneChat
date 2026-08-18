use std::time::{Duration, Instant};

use gpui::{Context, ScrollHandle};
use tokio_util::sync::CancellationToken;

use super::{
    content::{output_sources, prompts_include_text, render_prompt},
    languages::{resolved_source_language, same_language},
    state::ActiveTranslation,
};
use crate::{
    application::{
        context_usage::estimate_input_tokens,
        generation::{apply_event, interrupted_event},
    },
    desktop::app::{CachedMarkdown, OneChat, Page},
    domain::{
        AssistantBlock, AssistantResponse, GenerationConfig, GenerationRequest, Message,
        MessageStatus, Model, RequestInfo, new_id,
    },
    markdown::MarkdownDocument,
    providers,
};

const TRANSLATION_CONVERSATION_ID: &str = "translation-playground";
const EVENT_FLUSH_INTERVAL: Duration = Duration::from_millis(40);

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

        self.translation.next_operation_id =
            self.translation.next_operation_id.wrapping_add(1).max(1);
        let operation_id = self.translation.next_operation_id;
        let cancellation = CancellationToken::new();
        self.translation.active = Some(ActiveTranslation {
            id: operation_id,
            cancellation: cancellation.clone(),
        });
        self.translation.response = Some(response.clone());
        self.translation.request = Some(request.clone());
        self.translation.error = None;
        self.chat
            .thinking_started_at
            .insert(request.id.clone(), Instant::now());
        self.chat
            .thinking_scrolls
            .insert(response.id.clone(), ScrollHandle::new());
        cx.notify();

        let (sender, receiver) = async_channel::bounded(256);
        self.services
            .runtime
            .spawn(providers::generate(provider_request, sender, cancellation));

        let request_id = request.id.clone();
        cx.spawn(async move |this, cx| {
            let started = Instant::now();
            let mut terminal = false;
            loop {
                cx.background_executor().timer(EVENT_FLUSH_INTERVAL).await;
                let mut events = Vec::new();
                while let Ok(event) = receiver.try_recv() {
                    events.push(event);
                }
                if events.is_empty() && receiver.is_closed() && !terminal {
                    events.push(interrupted_event());
                }
                if events.is_empty() {
                    if terminal {
                        break;
                    }
                    continue;
                }

                let mut finished_reasoning = Vec::new();
                for event in events {
                    let outcome =
                        apply_event(event, &mut response, &mut request, started.elapsed());
                    terminal |= outcome.terminal;
                    finished_reasoning.extend(outcome.finished_reasoning_id);
                }
                let parsed = terminal.then(|| {
                    output_sources(&response)
                        .into_iter()
                        .map(|(id, source)| {
                            let document = MarkdownDocument::parse(&source);
                            (id, source, document)
                        })
                        .collect::<Vec<_>>()
                });
                let response_snapshot = response.clone();
                let request_snapshot = request.clone();
                let _ = this.update(cx, |this, cx| {
                    let current = this
                        .translation
                        .active
                        .as_ref()
                        .is_some_and(|active| active.id == operation_id);
                    if !current {
                        return;
                    }
                    for block in &response_snapshot.blocks {
                        if let AssistantBlock::Reasoning { id, .. } = block {
                            this.chat.thinking_scrolls.entry(id.clone()).or_default();
                        }
                    }
                    for id in finished_reasoning.drain(..) {
                        this.finish_thinking(id);
                    }
                    if let Some(parsed) = parsed {
                        for (id, source, document) in parsed {
                            this.chat
                                .markdown_documents
                                .insert(id, CachedMarkdown { source, document });
                        }
                    }
                    this.translation.response = Some(response_snapshot.clone());
                    this.translation.request = Some(request_snapshot.clone());
                    this.translation.result_scroll.scroll_to_bottom();
                    if terminal {
                        this.chat.thinking_started_at.remove(&request_id);
                        this.translation.active = None;
                    }
                    cx.notify();
                });
                if terminal {
                    break;
                }
            }
            let _ = this.update(cx, |this, cx| {
                this.chat.thinking_started_at.remove(&request_id);
                if this
                    .translation
                    .active
                    .as_ref()
                    .is_some_and(|active| active.id == operation_id)
                {
                    this.translation.active = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn stop_translation(&mut self, cx: &mut Context<Self>) {
        if let Some(active) = &self.translation.active {
            active.cancellation.cancel();
            cx.notify();
        }
    }

    pub(crate) fn run_translation_action(&mut self, cx: &mut Context<Self>) {
        if self.navigation.page == Page::Translate {
            self.start_translation(cx);
        }
    }
}
