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
                self.refresh_markdown_documents(cx);
            }
            Err(error) => self.data.error = Some(format!("Storage error: {error}")),
        }
    }

    pub(super) fn refresh_markdown_documents(&mut self, cx: &mut Context<Self>) {
        self.chat.markdown_documents.retain(|message_id, cached| {
            self.data.snapshot.current_messages.iter().any(|message| {
                message.id == *message_id
                    && message.role == MessageRole::Assistant
                    && message.content == cached.source
            })
        });
        let pending = self
            .data
            .snapshot
            .current_messages
            .iter()
            .filter(|message| {
                message.role == MessageRole::Assistant
                    && !self.chat.markdown_documents.contains_key(&message.id)
            })
            .map(|message| (message.id.clone(), message.content.clone()))
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
                        .current_messages
                        .iter()
                        .any(|message| message.id == id && message.content == source)
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

    pub(crate) fn markdown_for(&self, message: &Message) -> Option<&MarkdownDocument> {
        self.chat
            .markdown_documents
            .get(&message.id)
            .filter(|cached| cached.source == message.content)
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
        self.chat.expanded_error_ids.clear();
        self.chat.expanded_thinking_ids.clear();
        self.chat.message_editor = None;
        self.chat.follow_latest = true;
        self.chat.message_scroll = ScrollHandle::new();
        self.chat.message_scroll.scroll_to_bottom();
        self.chat.generation_config_editor = None;
        self.chat.parameter_error = None;
        self.sync_generation_config_editor(cx);
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
        self.data
            .snapshot
            .models
            .iter()
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
        let selected_id = self.selected_model().map(|model| model.id.as_str());
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

    pub(crate) fn current_messages(&self) -> &[Message] {
        &self.data.snapshot.current_messages
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
        self.data.snapshot.current_requests.first()
    }

    pub(crate) fn request_for_message(&self, message: &Message) -> Option<&RequestInfo> {
        let request_id = message.request_id.as_deref()?;
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

    pub(crate) fn is_latest_assistant(&self, message_id: &str) -> bool {
        self.data
            .snapshot
            .current_messages
            .iter()
            .rev()
            .find(|message| message.role == MessageRole::Assistant)
            .is_some_and(|message| message.id == message_id)
    }
}
