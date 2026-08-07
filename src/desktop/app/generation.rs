use super::*;

enum AutoTitleUpdate {
    Claimed,
    Finished(Option<String>),
    ClaimFailed(String),
}

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
        let (conversation, provider, model) = match self.generation_target(None) {
            Ok(target) => target,
            Err(error) => {
                self.data.error = Some(error);
                cx.notify();
                return;
            }
        };
        let parent_response_id = self
            .active_leaf_turn()
            .and_then(|turn| turn.continuation_response_id.clone());
        if self.active_leaf_turn().is_some_and(|turn| {
            parent_response_id
                .as_deref()
                .and_then(|id| turn.response(id))
                .is_none_or(|response| {
                    response.status != MessageStatus::Completed || response.content.is_empty()
                })
        }) {
            self.data.error = Some("Choose a completed response before continuing.".into());
            cx.notify();
            return;
        }
        let prepared = PreparedGeneration::new(
            &conversation,
            &provider,
            &model,
            &self.data.snapshot.current_turns,
            parent_response_id,
            prompt,
        );
        self.begin_prepared_generation(prepared, cx);
    }

    pub(crate) fn start_additional_response(
        &mut self,
        turn_id: String,
        model_id: String,
        cx: &mut Context<Self>,
    ) {
        let (conversation, provider, model) = match self.generation_target(Some(&model_id)) {
            Ok(target) => target,
            Err(error) => {
                self.data.error = Some(error);
                cx.notify();
                return;
            }
        };
        let Some(turn) = self
            .data
            .snapshot
            .current_turns
            .iter()
            .find(|turn| turn.id == turn_id)
            .cloned()
        else {
            return;
        };
        if turn
            .responses
            .iter()
            .any(|response| response.model_id == model.id)
        {
            self.data.error = Some("This model has already answered this message.".into());
            cx.notify();
            return;
        }
        let prepared = PreparedGeneration::additional(
            &conversation.id,
            &provider,
            &model,
            &self.data.snapshot.current_turns,
            &turn,
        );
        self.begin_prepared_generation(prepared, cx);
    }

    pub(super) fn generation_target(
        &self,
        model_id: Option<&str>,
    ) -> Result<(Conversation, Provider, Model), String> {
        let conversation = self
            .current_conversation()
            .cloned()
            .ok_or_else(|| "Create or select a conversation first.".to_string())?;
        let model = if let Some(model_id) = model_id {
            self.data
                .snapshot
                .models
                .iter()
                .find(|model| model.id == model_id)
        } else {
            self.current_model()
        }
        .cloned()
        .ok_or_else(|| "Choose a model before sending.".to_string())?;
        if !model.capabilities.streaming {
            return Err("The selected model does not support streaming.".into());
        }
        let provider = self
            .provider_for_model(&model)
            .cloned()
            .ok_or_else(|| "The selected model has no provider.".to_string())?;
        if !provider.enabled {
            return Err("The selected provider is disabled.".into());
        }
        Ok((conversation, provider, model))
    }

    pub(crate) fn regenerate_assistant(&mut self, response_id: String, cx: &mut Context<Self>) {
        let Some((turn, response)) = self
            .response(&response_id)
            .map(|(turn, response)| (turn.clone(), response.clone()))
        else {
            return;
        };
        if !self.is_latest_turn(&turn.id) {
            self.data.error = Some("Only responses in the latest turn can be regenerated.".into());
            cx.notify();
            return;
        }
        let (conversation, provider, model) = match self.generation_target(Some(&response.model_id))
        {
            Ok(target) => target,
            Err(error) => {
                self.data.error = Some(error);
                cx.notify();
                return;
            }
        };
        let prepared = PreparedGeneration::regenerate(
            &conversation.id,
            &provider,
            &model,
            &self.data.snapshot.current_turns,
            &turn,
            &response,
        );
        self.begin_prepared_generation(prepared, cx);
    }

    pub(super) fn begin_prepared_generation(
        &mut self,
        prepared: PreparedGeneration,
        cx: &mut Context<Self>,
    ) {
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
            prepared.response.id.clone(),
            cancellation.clone(),
        ) {
            return;
        }

        let turn_id = prepared.request_info.turn_id.clone();
        let response_id = prepared.response.id.clone();
        let scroll_to_bottom =
            matches!(&prepared.start, GenerationStart::NewTurn(_)) || self.is_latest_turn(&turn_id);
        if !matches!(&prepared.start, GenerationStart::NewTurn(_)) {
            self.chat
                .visible_response_ids
                .insert(turn_id, response_id.clone());
        }
        if scroll_to_bottom {
            self.chat.message_scroll_motion.cancel();
            self.chat.follow_latest = true;
            self.chat.message_scroll.scroll_to_bottom();
        }
        self.chat.message_editor = None;
        self.chat.thinking_expansion_overrides.remove(&response_id);
        self.chat.thinking_motions.remove(&response_id);
        self.chat
            .thinking_scrolls
            .insert(response_id, ScrollHandle::new());
        cx.notify();

        let persisted = prepared.clone();
        let storage = self.services.storage.clone();
        let previous = std::mem::replace(&mut self.data.storage_task, Task::ready(()));
        self.data.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move {
                    match &persisted.start {
                        GenerationStart::NewTurn(turn) => {
                            storage.begin_turn(turn, &persisted.request_info)?;
                        }
                        GenerationStart::AddResponse { turn_id } => storage.begin_response(
                            &persisted.request_info.conversation_id,
                            turn_id,
                            &persisted.response,
                            &persisted.request_info,
                        )?,
                        GenerationStart::RetryResponse { turn_id } => storage.begin_regeneration(
                            &persisted.request_info.conversation_id,
                            turn_id,
                            &persisted.response,
                            &persisted.request_info,
                        )?,
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
        let root_user_message = self
            .data
            .snapshot
            .current_turns
            .iter()
            .find(|turn| turn.id == prepared.request_info.turn_id)
            .filter(|turn| turn.parent_response_id.is_none())
            .map(|turn| turn.user.content.clone());
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
                        let response = snapshot.response;
                        let request = snapshot.request;
                        let terminal = snapshot.terminal;
                        let thinking_finished = snapshot.thinking_finished;
                        let parsed_markdown = if response.content != last_markdown_source {
                            last_markdown_source.clone_from(&response.content);
                            let source = response.content.clone();
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
                            let visible = this.update_generation_snapshot(
                                &conversation_id,
                                &response,
                                &request,
                            );
                            if visible && thinking_finished && !response.thinking.is_empty() {
                                this.finish_thinking(response.id.clone());
                            }
                            if let Some((source, document)) = parsed_markdown
                                && this
                                    .response(&response.id)
                                    .is_some_and(|(_, stored)| stored.content == source)
                            {
                                this.chat.markdown_documents.insert(
                                    response.id.clone(),
                                    CachedMarkdown { source, document },
                                );
                            }
                            if terminal {
                                this.chat.generations.finish(&conversation_id, &request_id);
                                if let Some(user_message) = root_user_message.clone() {
                                    this.start_auto_title(
                                        conversation_id.clone(),
                                        user_message,
                                        &response,
                                        cx,
                                    );
                                }
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

    fn start_auto_title(
        &mut self,
        conversation_id: String,
        user_message: String,
        response: &AssistantResponse,
        cx: &mut Context<Self>,
    ) {
        if !self.data.snapshot.settings.auto_title_enabled
            || response.status != MessageStatus::Completed
            || response.content.trim().is_empty()
        {
            return;
        }
        if !self.data.snapshot.conversations.iter().any(|conversation| {
            conversation.id == conversation_id
                && conversation.auto_title_state == AutoTitleState::Pending
        }) {
            return;
        }

        let system_prompt = self
            .data
            .snapshot
            .settings
            .title_generation_system_prompt
            .trim()
            .to_string();
        let target = self
            .title_generation_model()
            .filter(|model| self.model_availability(model).is_ok())
            .and_then(|model| {
                self.provider_for_model(model)
                    .map(|provider| (provider.clone(), model.clone()))
            })
            .filter(|_| !system_prompt.is_empty());
        let assistant_response = response.content.clone();
        let storage = self.services.storage.clone();
        let claim_id = conversation_id.clone();
        let (sender, receiver) = async_channel::bounded::<AutoTitleUpdate>(2);
        self.services.runtime.spawn(async move {
            let claimed = tokio::task::spawn_blocking(move || {
                storage
                    .claim_auto_title(&claim_id)
                    .map_err(|error| error.to_string())
            })
            .await;
            match claimed {
                Ok(Ok(true)) => {
                    if sender.send(AutoTitleUpdate::Claimed).await.is_err() {
                        return;
                    }
                }
                Ok(Ok(false)) => return,
                Ok(Err(error)) => {
                    let _ = sender.send(AutoTitleUpdate::ClaimFailed(error)).await;
                    return;
                }
                Err(error) => {
                    let _ = sender
                        .send(AutoTitleUpdate::ClaimFailed(format!(
                            "automatic title claim task failed: {error}"
                        )))
                        .await;
                    return;
                }
            }

            let title = match target {
                Some((provider, model)) => generate_title(
                    provider,
                    model,
                    system_prompt,
                    user_message,
                    assistant_response,
                )
                .await
                .ok(),
                None => None,
            };
            let _ = sender.send(AutoTitleUpdate::Finished(title)).await;
        });

        cx.spawn(async move |this, cx| {
            while let Ok(update) = receiver.recv().await {
                let finished = matches!(
                    &update,
                    AutoTitleUpdate::Finished(_) | AutoTitleUpdate::ClaimFailed(_)
                );
                let _ = this.update(cx, |this, cx| match update {
                    AutoTitleUpdate::Claimed => {
                        if let Some(conversation) = this
                            .data
                            .snapshot
                            .conversations
                            .iter_mut()
                            .find(|conversation| {
                                conversation.id == conversation_id
                                    && conversation.auto_title_state == AutoTitleState::Pending
                            })
                        {
                            conversation.auto_title_state = AutoTitleState::Running;
                            cx.notify();
                        }
                    }
                    AutoTitleUpdate::Finished(title) => {
                        let title = this
                            .data
                            .snapshot
                            .settings
                            .auto_title_enabled
                            .then_some(title)
                            .flatten();
                        if let Some(new_title) = title.as_ref()
                            && let Some(conversation) = this
                                .data
                                .snapshot
                                .conversations
                                .iter()
                                .find(|conversation| {
                                    conversation.id == conversation_id
                                        && conversation.auto_title_state == AutoTitleState::Running
                                })
                            && conversation.title != *new_title
                        {
                            this.chat.pending_title_transitions.insert(
                                conversation_id.clone(),
                                PendingTitleTransition {
                                    old_title: conversation.title.clone(),
                                    new_title: new_title.clone(),
                                },
                            );
                        }
                        let conversation_id = conversation_id.clone();
                        this.mutate_and_reload(
                            move |storage| {
                                storage
                                    .finish_auto_title(&conversation_id, title.as_deref())
                                    .map(|_| ())
                            },
                            cx,
                        );
                    }
                    AutoTitleUpdate::ClaimFailed(error) => {
                        this.data.error = Some(format!("Could not start automatic title: {error}"));
                        cx.notify();
                    }
                });
                if finished {
                    break;
                }
            }
        })
        .detach();
    }

    fn update_generation_snapshot(
        &mut self,
        conversation_id: &str,
        response: &AssistantResponse,
        request: &RequestInfo,
    ) -> bool {
        if self
            .data
            .snapshot
            .settings
            .current_conversation_id
            .as_deref()
            != Some(conversation_id)
        {
            return false;
        }
        let thinking_grew = self
            .response(&response.id)
            .is_none_or(|(_, stored)| stored.thinking.len() < response.thinking.len());
        if let Some(turn) = self
            .data
            .snapshot
            .current_turns
            .iter_mut()
            .find(|turn| turn.id == request.turn_id)
        {
            if let Some(stored) = turn
                .responses
                .iter_mut()
                .find(|stored| stored.id == response.id)
            {
                *stored = response.clone();
            }
            let continuation_is_unusable = turn
                .continuation_response_id
                .as_deref()
                .and_then(|id| turn.response(id))
                .is_none_or(|response| {
                    response.status != MessageStatus::Completed || response.content.is_empty()
                });
            if response.status == MessageStatus::Completed
                && !response.content.is_empty()
                && continuation_is_unusable
            {
                turn.continuation_response_id = Some(response.id.clone());
            }
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
                .entry(response.id.clone())
                .or_default()
                .scroll_to_bottom();
        }
        if self.chat.follow_latest && self.is_latest_turn(&request.turn_id) {
            self.chat.message_scroll.scroll_to_bottom();
        }
        true
    }
}
