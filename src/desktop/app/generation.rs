use super::*;

impl OneChat {
    pub(crate) fn is_current_generating(&self) -> bool {
        self.current_conversation()
            .is_some_and(|conversation| self.chat.generations.is_active(&conversation.id))
    }

    pub(crate) fn send_composer(&mut self, cx: &mut Context<Self>) {
        let prompt = self
            .chat
            .composer
            .update(cx, |composer, cx| composer.take_text(cx));
        if let Some(prompt) = prompt {
            self.start_generation(prompt, cx);
        }
    }

    pub(crate) fn stop_current_generation(&mut self, cx: &mut Context<Self>) {
        if let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone()) {
            self.chat.generations.stop(&conversation_id);
            cx.notify();
        }
    }

    pub(super) fn start_generation(&mut self, prompt: String, cx: &mut Context<Self>) {
        let (conversation, provider, model) = match self.generation_target() {
            Ok(target) => target,
            Err(error) => {
                self.data.error = Some(error);
                cx.notify();
                return;
            }
        };
        let prepared = PreparedGeneration::new(
            &conversation,
            &provider,
            &model,
            &self.data.snapshot.current_messages,
            prompt,
        );
        self.begin_prepared_generation(prepared, cx);
    }

    fn generation_target(&self) -> Result<(Conversation, Provider, Model), String> {
        let conversation = self
            .current_conversation()
            .cloned()
            .ok_or_else(|| "Create or select a conversation first.".to_string())?;
        let model = self
            .current_model()
            .cloned()
            .ok_or_else(|| "Choose a model before sending.".to_string())?;
        if !model.capabilities.streaming {
            return Err("The selected model does not support streaming.".into());
        }
        let provider = self
            .current_provider()
            .cloned()
            .ok_or_else(|| "The selected model has no provider.".to_string())?;
        if !provider.enabled {
            return Err("The selected provider is disabled.".into());
        }
        Ok((conversation, provider, model))
    }

    pub(crate) fn regenerate_assistant(&mut self, message_id: String, cx: &mut Context<Self>) {
        let (conversation, provider, model) = match self.generation_target() {
            Ok(target) => target,
            Err(error) => {
                self.data.error = Some(error);
                cx.notify();
                return;
            }
        };
        let Some(latest_assistant) = self
            .data
            .snapshot
            .current_messages
            .iter()
            .rev()
            .find(|message| message.role == MessageRole::Assistant)
        else {
            return;
        };
        if latest_assistant.id != message_id {
            self.data.error = Some("Only the latest assistant response can be regenerated.".into());
            cx.notify();
            return;
        }
        let Some(index) = self
            .data
            .snapshot
            .current_messages
            .iter()
            .position(|message| message.id == message_id)
        else {
            return;
        };
        let previous_assistant = self.data.snapshot.current_messages[index].clone();
        let prepared = PreparedGeneration::regenerate(
            &conversation,
            &provider,
            &model,
            &self.data.snapshot.current_messages[..index],
            &previous_assistant,
        );
        self.begin_prepared_generation(prepared, cx);
    }

    fn begin_prepared_generation(&mut self, prepared: PreparedGeneration, cx: &mut Context<Self>) {
        let conversation_id = prepared.request_info.conversation_id.clone();
        if self.chat.generations.is_active(&conversation_id) {
            self.data.error = Some("This conversation already has an active generation.".into());
            cx.notify();
            return;
        }
        let cancellation = CancellationToken::new();
        if !self.chat.generations.start(
            conversation_id.clone(),
            prepared.request_info.id.clone(),
            prepared.assistant.id.clone(),
            cancellation.clone(),
        ) {
            return;
        }
        self.chat.follow_latest = true;
        self.chat.message_editor = None;
        self.chat
            .collapsed_thinking_ids
            .remove(&prepared.assistant.id);
        self.chat.thinking_motions.remove(&prepared.assistant.id);
        self.chat
            .thinking_scrolls
            .insert(prepared.assistant.id.clone(), ScrollHandle::new());
        self.chat.message_scroll.scroll_to_bottom();
        cx.notify();

        let persisted = prepared.clone();
        let storage = self.services.storage.clone();
        let previous = std::mem::replace(&mut self.data.storage_task, Task::ready(()));
        self.data.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move {
                    if let Some(user) = persisted.user.as_ref() {
                        storage.begin_generation(
                            user,
                            &persisted.assistant,
                            &persisted.request_info,
                        )?;
                    } else {
                        storage
                            .begin_regeneration(&persisted.assistant, &persisted.request_info)?;
                    }
                    storage.load_snapshot()
                })
                .await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(snapshot) => {
                    this.data.snapshot = snapshot;
                    this.data.error = None;
                    this.chat.selected_request_id = Some(prepared.request_info.id.clone());
                    this.refresh_markdown_documents(cx);
                    this.launch_generation(prepared, cancellation, cx);
                    cx.notify();
                }
                Err(error) => {
                    this.chat
                        .generations
                        .finish(&conversation_id, &prepared.request_info.id);
                    this.data.error = Some(format!("Could not start generation: {error}"));
                    cx.notify();
                }
            });
        });
    }

    fn launch_generation(
        &mut self,
        prepared: PreparedGeneration,
        cancellation: CancellationToken,
        cx: &mut Context<Self>,
    ) {
        let conversation_id = prepared.request_info.conversation_id.clone();
        let request_id = prepared.request_info.id.clone();
        self.chat
            .thinking_started_at
            .insert(request_id.clone(), Instant::now());
        let storage = self.services.storage.clone();
        let (sender, receiver) = async_channel::bounded(32);
        self.services
            .runtime
            .spawn(run_generation(prepared, storage, cancellation, sender));

        let timer_request_id = request_id.clone();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let ticking = this
                    .update(cx, |this, cx| {
                        let ticking = this
                            .chat
                            .thinking_started_at
                            .contains_key(&timer_request_id);
                        if ticking {
                            cx.notify();
                        }
                        ticking
                    })
                    .unwrap_or(false);
                if !ticking {
                    break;
                }
            }
        })
        .detach();

        let cleanup_request_id = request_id.clone();
        cx.spawn(async move |this, cx| {
            let mut last_markdown_source = String::new();
            while let Ok(update) = receiver.recv().await {
                match update {
                    GenerationUpdate::PersistenceFailed(error) => {
                        let _ = this.update(cx, |this, cx| {
                            this.data.error = Some(format!("Could not save generation: {error}"));
                            cx.notify();
                        });
                    }
                    GenerationUpdate::Snapshot(snapshot) => {
                        let assistant = snapshot.assistant;
                        let request = snapshot.request;
                        let terminal = snapshot.terminal;
                        let parsed_markdown = if assistant.content != last_markdown_source {
                            last_markdown_source.clone_from(&assistant.content);
                            let source = assistant.content.clone();
                            Some(
                                cx.background_spawn(async move {
                                    let document = MarkdownDocument::parse(&source);
                                    (source, document)
                                })
                                .await,
                            )
                        } else {
                            None
                        };
                        let _ = this.update(cx, |this, cx| {
                            if request.thinking_duration_ms.is_some() || terminal {
                                this.chat.thinking_started_at.remove(&request.id);
                            }
                            this.update_generation_snapshot(&conversation_id, &assistant, &request);
                            if let Some((source, document)) = parsed_markdown
                                && this.data.snapshot.current_messages.iter().any(|message| {
                                    message.id == assistant.id && message.content == source
                                })
                            {
                                this.chat.markdown_documents.insert(
                                    assistant.id.clone(),
                                    CachedMarkdown { source, document },
                                );
                            }
                            if terminal {
                                this.chat.generations.finish(&conversation_id, &request_id);
                            }
                            cx.notify();
                        });
                        if terminal {
                            break;
                        }
                    }
                }
            }
            let _ = this.update(cx, |this, _| {
                this.chat.thinking_started_at.remove(&cleanup_request_id);
            });
        })
        .detach();
    }

    fn update_generation_snapshot(
        &mut self,
        conversation_id: &str,
        assistant: &Message,
        request: &RequestInfo,
    ) {
        if self
            .data
            .snapshot
            .settings
            .current_conversation_id
            .as_deref()
            != Some(conversation_id)
        {
            return;
        }
        let thinking_grew = self
            .data
            .snapshot
            .current_messages
            .iter()
            .find(|message| message.id == assistant.id)
            .is_none_or(|message| message.thinking.len() < assistant.thinking.len());
        if let Some(message) = self
            .data
            .snapshot
            .current_messages
            .iter_mut()
            .find(|message| message.id == assistant.id)
        {
            *message = assistant.clone();
        }
        if let Some(info) = self
            .data
            .snapshot
            .current_requests
            .iter_mut()
            .find(|info| info.id == request.id)
        {
            *info = request.clone();
        }
        if thinking_grew {
            self.chat
                .thinking_scrolls
                .entry(assistant.id.clone())
                .or_default()
                .scroll_to_bottom();
        }
        if self.chat.follow_latest {
            self.chat.message_scroll.scroll_to_bottom();
        }
    }
}
