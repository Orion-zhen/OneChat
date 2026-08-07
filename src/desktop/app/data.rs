use super::*;

impl OneChat {
    pub(super) fn load_startup_snapshot(&mut self, cx: &mut Context<Self>) {
        let previous = std::mem::replace(&mut self.data.storage_task, Task::ready(()));
        let storage = self.services.storage.clone();
        self.data.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move { storage.load_startup_snapshot() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.data.loading = false;
                this.apply_snapshot(result, cx);
                cx.notify();
            });
        });
    }

    fn apply_snapshot(&mut self, result: StorageResult<StorageSnapshot>, cx: &mut Context<Self>) {
        match result {
            Ok(snapshot) => {
                let previous_conversation_id =
                    self.data.snapshot.settings.current_conversation_id.clone();
                let conversation_changed =
                    previous_conversation_id != snapshot.settings.current_conversation_id;
                self.data.snapshot = snapshot;
                self.data.error = None;
                if conversation_changed {
                    self.reset_conversation_ui(cx);
                    if self.current_conversation().is_some() {
                        self.navigation.pending_focus = Some(PendingFocus::Composer);
                    }
                } else {
                    self.sync_generation_config_editor(cx);
                }
                self.sync_thinking_scrolls();
                self.refresh_markdown_documents(cx);
            }
            Err(error) => self.data.error = Some(format!("Storage error: {error}")),
        }
    }

    pub(super) fn refresh_markdown_documents(&mut self, cx: &mut Context<Self>) {
        self.chat.markdown_documents.retain(|response_id, cached| {
            self.data
                .snapshot
                .current_turns
                .iter()
                .flat_map(|turn| &turn.responses)
                .any(|response| response.id == *response_id && response.content == cached.source)
        });
        let pending = self
            .data
            .snapshot
            .current_turns
            .iter()
            .flat_map(|turn| &turn.responses)
            .filter(|response| !self.chat.markdown_documents.contains_key(&response.id))
            .map(|response| (response.id.clone(), response.content.clone()))
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let parsed = cx
                .background_spawn(async move {
                    pending
                        .into_iter()
                        .map(|(id, source)| {
                            let document = MarkdownDocument::parse(&source);
                            (id, source, document)
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                for (id, source, document) in parsed {
                    if this
                        .data
                        .snapshot
                        .current_turns
                        .iter()
                        .flat_map(|turn| &turn.responses)
                        .any(|response| response.id == id && response.content == source)
                    {
                        this.chat
                            .markdown_documents
                            .insert(id, CachedMarkdown { source, document });
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn markdown_for(&self, response: &AssistantResponse) -> Option<&MarkdownDocument> {
        self.chat
            .markdown_documents
            .get(&response.id)
            .filter(|cached| cached.source == response.content)
            .map(|cached| &cached.document)
    }

    pub(super) fn sync_generation_config_editor(&mut self, cx: &mut Context<Self>) {
        let conversation = self.current_conversation().cloned();
        match conversation {
            Some(conversation)
                if self
                    .chat
                    .generation_config_editor
                    .as_ref()
                    .is_none_or(|editor| !editor.is_for(&conversation.id)) =>
            {
                self.chat.generation_config_editor =
                    Some(GenerationConfigEditor::new(&conversation, cx));
                self.chat.parameter_error = None;
            }
            None => {
                self.chat.generation_config_editor = None;
                self.chat.parameter_error = None;
            }
            Some(_) => {}
        }
    }

    pub(super) fn reset_conversation_ui(&mut self, cx: &mut Context<Self>) {
        self.chat.draft_model_id = None;
        self.chat.system_prompt_mode = SystemPromptMode::Compact;
        self.chat.system_prompt_editor = None;
        self.overlays.command_palette_open = false;
        self.overlays.model_picker_open = false;
        self.chat.selected_request_id = None;
        self.chat.visible_response_ids.clear();
        self.overlays.response_model_turn_id = None;
        self.chat.expanded_error_ids.clear();
        self.chat.thinking_expansion_overrides.clear();
        self.chat.message_editor = None;
        self.chat.follow_latest = true;
        self.chat.message_scroll_motion.cancel();
        self.chat.message_scroll = ScrollHandle::new();
        self.chat.message_scroll.scroll_to_bottom();
        self.chat.thinking_scrolls.clear();
        self.chat.thinking_motions.clear();
        self.chat.generation_config_editor = None;
        self.chat.generation_config_save_revision =
            self.chat.generation_config_save_revision.wrapping_add(1);
        self.chat.parameter_error = None;
        self.sync_generation_config_editor(cx);
    }

    fn sync_thinking_scrolls(&mut self) {
        self.chat.thinking_motions.retain(|message_id, _| {
            self.data
                .snapshot
                .current_turns
                .iter()
                .flat_map(|turn| &turn.responses)
                .any(|response| response.id == *message_id)
        });
        self.chat.thinking_scrolls.retain(|message_id, _| {
            self.data
                .snapshot
                .current_turns
                .iter()
                .flat_map(|turn| &turn.responses)
                .any(|response| response.id == *message_id)
        });
        for response in self
            .data
            .snapshot
            .current_turns
            .iter()
            .flat_map(|turn| &turn.responses)
        {
            self.chat
                .thinking_scrolls
                .entry(response.id.clone())
                .or_default();
        }
    }

    pub(super) fn mutate_and_reload<F>(&mut self, operation: F, cx: &mut Context<Self>)
    where
        F: FnOnce(&Storage) -> StorageResult<()> + Send + 'static,
    {
        let previous = std::mem::replace(&mut self.data.storage_task, Task::ready(()));
        let storage = self.services.storage.clone();
        self.data.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move {
                    operation(&storage)?;
                    storage.load_snapshot()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.apply_snapshot(result, cx);
                cx.notify();
            });
        });
    }

    pub(super) fn save_settings(&mut self, cx: &mut Context<Self>) {
        let previous = std::mem::replace(&mut self.data.storage_task, Task::ready(()));
        let storage = self.services.storage.clone();
        let settings = self.data.snapshot.settings.clone();
        self.data.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move { storage.save_settings(&settings) })
                .await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.data.error = Some(format!("Could not save settings: {error}"));
                    cx.notify();
                });
            }
        });
    }

    pub(crate) fn current_conversation(&self) -> Option<&Conversation> {
        let id = self
            .data
            .snapshot
            .settings
            .current_conversation_id
            .as_deref()?;
        self.data
            .snapshot
            .conversations
            .iter()
            .find(|conversation| conversation.id == id)
    }

    pub(crate) fn primary_model(&self) -> Option<&Model> {
        let model_id = self.data.snapshot.settings.primary_model_id.as_deref()?;
        self.data
            .snapshot
            .models
            .iter()
            .find(|model| model.id == model_id)
    }

    pub(crate) fn current_model(&self) -> Option<&Model> {
        let conversation = self.current_conversation()?;
        conversation
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

    pub(crate) fn selected_model(&self) -> Option<&Model> {
        self.current_model()
            .or_else(|| {
                let model_id = self.chat.draft_model_id.as_deref()?;
                self.data
                    .snapshot
                    .models
                    .iter()
                    .find(|model| model.id == model_id)
            })
            .or_else(|| self.primary_model())
    }

    pub(crate) fn current_provider(&self) -> Option<&Provider> {
        let provider_id = &self.current_model()?.provider_id;
        self.data
            .snapshot
            .providers
            .iter()
            .find(|provider| &provider.id == provider_id)
    }

    pub(crate) fn provider_for_model(&self, model: &Model) -> Option<&Provider> {
        self.data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == model.provider_id)
    }

    pub(crate) fn model_availability(&self, model: &Model) -> Result<(), &'static str> {
        let Some(provider) = self.provider_for_model(model) else {
            return Err("Missing provider");
        };
        if !provider.enabled {
            return Err("Provider disabled");
        }
        if !model.capabilities.streaming {
            return Err("Streaming disabled");
        }
        Ok(())
    }

    pub(crate) fn filtered_commands(&self) -> Vec<PaletteCommand> {
        PaletteCommand::ALL
            .into_iter()
            .filter(|command| command.matches(&self.overlays.command_query))
            .collect()
    }

    pub(crate) fn filtered_models(&self) -> Vec<&Model> {
        let query = self.overlays.model_query.trim().to_lowercase();
        let replied_model_ids = self
            .overlays
            .response_model_turn_id
            .as_deref()
            .and_then(|turn_id| {
                self.data
                    .snapshot
                    .current_turns
                    .iter()
                    .find(|turn| turn.id == turn_id)
            })
            .map(|turn| {
                turn.responses
                    .iter()
                    .map(|response| response.model_id.as_str())
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        self.data
            .snapshot
            .models
            .iter()
            .filter(|model| !replied_model_ids.contains(model.id.as_str()))
            .filter(|model| {
                if query.is_empty() {
                    return true;
                }
                let provider = self
                    .provider_for_model(model)
                    .map(|provider| provider.name.as_str())
                    .unwrap_or_default();
                [
                    model.display_name.as_str(),
                    model.remote_id.as_str(),
                    provider,
                ]
                .into_iter()
                .any(|value| value.to_lowercase().contains(&query))
            })
            .collect()
    }

    pub(super) fn initial_model_selection(&self) -> usize {
        let models = self.filtered_models();
        let selected_id = self
            .overlays
            .response_model_turn_id
            .is_none()
            .then(|| self.selected_model().map(|model| model.id.as_str()))
            .flatten();
        models
            .iter()
            .position(|model| {
                selected_id == Some(model.id.as_str()) && self.model_availability(model).is_ok()
            })
            .or_else(|| {
                models
                    .iter()
                    .position(|model| self.model_availability(model).is_ok())
            })
            .unwrap_or(0)
    }

    pub(crate) fn current_turns(&self) -> Vec<&Turn> {
        active_turns(&self.data.snapshot.current_turns)
    }

    pub(crate) fn active_leaf_turn(&self) -> Option<&Turn> {
        self.current_turns().last().copied()
    }

    pub(crate) fn user_branches(&self, turn: &Turn) -> Vec<&Turn> {
        user_branches(&self.data.snapshot.current_turns, turn)
    }

    pub(crate) fn current_request(&self) -> Option<&RequestInfo> {
        let conversation = self.current_conversation()?;
        if let Some(active) = self.chat.generations.active_request(&conversation.id) {
            return self
                .data
                .snapshot
                .current_requests
                .iter()
                .find(|request| request.id == active.request_id);
        }
        self.active_leaf_turn()
            .and_then(|turn| {
                turn.continuation_response_id
                    .as_deref()
                    .and_then(|id| turn.response(id))
                    .or_else(|| turn.responses.first())
            })
            .and_then(|response| self.request_for_response(response))
    }

    pub(crate) fn request_for_response(
        &self,
        response: &AssistantResponse,
    ) -> Option<&RequestInfo> {
        let request_id = response.request_id.as_deref()?;
        self.data
            .snapshot
            .current_requests
            .iter()
            .find(|request| request.id == request_id)
    }

    pub(crate) fn inspected_request(&self) -> Option<&RequestInfo> {
        self.chat
            .selected_request_id
            .as_deref()
            .and_then(|id| {
                self.data
                    .snapshot
                    .current_requests
                    .iter()
                    .find(|request| request.id == id)
            })
            .or_else(|| self.current_request())
    }

    pub(crate) fn visible_response<'a>(&self, turn: &'a Turn) -> Option<&'a AssistantResponse> {
        self.chat
            .visible_response_ids
            .get(&turn.id)
            .and_then(|id| turn.response(id))
            .or_else(|| {
                turn.continuation_response_id
                    .as_deref()
                    .and_then(|id| turn.response(id))
            })
            .or_else(|| turn.responses.first())
    }

    pub(crate) fn response(&self, response_id: &str) -> Option<(&Turn, &AssistantResponse)> {
        self.data
            .snapshot
            .current_turns
            .iter()
            .find_map(|turn| turn.response(response_id).map(|response| (turn, response)))
    }

    pub(crate) fn is_latest_turn(&self, turn_id: &str) -> bool {
        self.active_leaf_turn()
            .is_some_and(|turn| turn.id == turn_id)
    }

    pub(crate) fn current_context_messages(&self) -> Vec<ChatMessage> {
        crate::application::generation::history_for_new_turn(&self.data.snapshot.current_turns)
    }
}
