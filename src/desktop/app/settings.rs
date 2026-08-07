use super::*;

impl OneChat {
    pub(crate) fn begin_message_width_drag(&mut self, ratio: f32, cx: &mut Context<Self>) {
        self.settings_ui.message_width_dragging = true;
        self.update_message_width_ratio(ratio, cx);
    }

    pub(crate) fn update_message_width_ratio(&mut self, ratio: f32, cx: &mut Context<Self>) {
        let ratio = ratio.clamp(
            crate::domain::MIN_MESSAGE_WIDTH_RATIO,
            crate::domain::MAX_MESSAGE_WIDTH_RATIO,
        );
        if (self.data.snapshot.settings.message_width_ratio - ratio).abs() < f32::EPSILON {
            return;
        }
        self.data.snapshot.settings.message_width_ratio = ratio;
        cx.notify();
    }

    pub(crate) fn finish_message_width_drag(&mut self, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.settings_ui.message_width_dragging) {
            return;
        }
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn toggle_auto_title_enabled(&mut self, cx: &mut Context<Self>) {
        self.data.snapshot.settings.auto_title_enabled =
            !self.data.snapshot.settings.auto_title_enabled;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn select_settings_section(
        &mut self,
        section: SettingsSection,
        cx: &mut Context<Self>,
    ) {
        if self.settings_ui.section == section {
            return;
        }
        self.settings_ui.section = section;
        self.settings_ui.default_model_menu = None;
        self.settings_ui.provider_editor = None;
        self.settings_ui.model_editor = None;
        self.settings_ui.prompt_editor = None;
        self.settings_ui.form_error = None;
        cx.notify();
    }

    pub(crate) fn toggle_default_model_menu(
        &mut self,
        role: DefaultModelRole,
        cx: &mut Context<Self>,
    ) {
        self.settings_ui.default_model_menu =
            (self.settings_ui.default_model_menu != Some(role)).then_some(role);
        cx.notify();
    }

    pub(crate) fn select_default_model(
        &mut self,
        role: DefaultModelRole,
        model_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(model_id) = model_id.as_deref() {
            let Some(model) = self
                .data
                .snapshot
                .models
                .iter()
                .find(|model| model.id == model_id)
            else {
                return;
            };
            if let Err(reason) = self.model_availability(model) {
                self.data.error = Some(format!("Model is unavailable: {reason}."));
                cx.notify();
                return;
            }
        } else if role == DefaultModelRole::Primary {
            return;
        }

        self.settings_ui.default_model_menu = None;
        let stored_id = match role {
            DefaultModelRole::Primary => &mut self.data.snapshot.settings.primary_model_id,
            DefaultModelRole::TitleGeneration => {
                &mut self.data.snapshot.settings.title_generation_model_id
            }
        };
        if *stored_id == model_id {
            cx.notify();
            return;
        }
        *stored_id = model_id;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn begin_edit_settings_prompt(
        &mut self,
        kind: SettingsPromptKind,
        cx: &mut Context<Self>,
    ) {
        let (content, placeholder) = match kind {
            SettingsPromptKind::ConversationDefault => (
                self.data.snapshot.settings.default_system_prompt.clone(),
                "Copied into each new conversation",
            ),
            SettingsPromptKind::TitleGeneration => (
                self.data
                    .snapshot
                    .settings
                    .title_generation_system_prompt
                    .clone(),
                "Used to generate automatic conversation titles",
            ),
        };
        let input = cx.new(|cx| Composer::multiline(content, placeholder, cx));
        cx.subscribe(&input, |this, _, event, cx| {
            if matches!(event, ComposerEvent::Cancel) {
                this.cancel_settings_prompt_edit(cx);
            }
        })
        .detach();
        self.settings_ui.prompt_editor = Some(SettingsPromptEditor { kind, input });
        self.navigation.pending_focus = Some(PendingFocus::SettingsPrompt);
        cx.notify();
    }

    pub(crate) fn cancel_settings_prompt_edit(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.prompt_editor = None;
        cx.notify();
    }

    pub(crate) fn save_settings_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.settings_ui.prompt_editor.as_ref() else {
            return;
        };
        let kind = editor.kind;
        let content = editor.input.read(cx).text().trim().to_string();
        if kind == SettingsPromptKind::TitleGeneration && content.is_empty() {
            self.data.error = Some("The title generation prompt cannot be empty.".into());
            cx.notify();
            return;
        }
        match kind {
            SettingsPromptKind::ConversationDefault => {
                self.data.snapshot.settings.default_system_prompt = content;
            }
            SettingsPromptKind::TitleGeneration => {
                self.data.snapshot.settings.title_generation_system_prompt = content;
            }
        }
        self.settings_ui.prompt_editor = None;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn reset_title_generation_prompt(&mut self, cx: &mut Context<Self>) {
        self.data.snapshot.settings.title_generation_system_prompt =
            DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT.into();
        if self
            .settings_ui
            .prompt_editor
            .as_ref()
            .is_some_and(|editor| editor.kind == SettingsPromptKind::TitleGeneration)
        {
            self.settings_ui.prompt_editor = None;
        }
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn begin_add_provider(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.section = SettingsSection::NewProvider;
        self.settings_ui.provider_editor = Some(ProviderEditor::new(None, cx));
        self.settings_ui.model_editor = None;
        self.settings_ui.form_error = None;
        cx.notify();
    }

    pub(crate) fn begin_edit_provider(&mut self, id: String, cx: &mut Context<Self>) {
        let provider = self
            .data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == id)
            .cloned();
        if let Some(provider) = provider {
            self.settings_ui.section = SettingsSection::Provider(provider.id.clone());
            self.settings_ui.provider_editor = Some(ProviderEditor::new(Some(provider), cx));
            self.settings_ui.model_editor = None;
            self.settings_ui.form_error = None;
            cx.notify();
        }
    }

    pub(crate) fn toggle_provider_kind_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.provider_editor {
            editor.toggle_kind_menu();
            cx.notify();
        }
    }

    pub(crate) fn select_provider_kind(&mut self, kind: ProviderKind, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.provider_editor {
            editor.select_kind(kind, cx);
            cx.notify();
        }
    }

    pub(crate) fn toggle_provider_enabled(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(mut provider) = self
            .data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == id)
            .cloned()
        else {
            return;
        };
        provider.enabled = !provider.enabled;
        provider.updated_at = now_timestamp();
        self.mutate_and_reload(move |storage| storage.update_provider(&provider), cx);
    }

    pub(crate) fn save_provider(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.settings_ui.provider_editor else {
            return;
        };
        let provider = match editor.build(cx) {
            Ok(provider) => provider,
            Err(error) => {
                self.settings_ui.form_error = Some(error);
                cx.notify();
                return;
            }
        };
        let insert = editor.is_new();
        self.settings_ui.section = SettingsSection::Provider(provider.id.clone());
        self.settings_ui.provider_editor = None;
        self.settings_ui.form_error = None;
        self.mutate_and_reload(
            move |storage| {
                if insert {
                    storage.insert_provider(&provider)
                } else {
                    storage.update_provider(&provider)
                }
            },
            cx,
        );
    }

    pub(crate) fn cancel_provider_editor(&mut self, cx: &mut Context<Self>) {
        if self.settings_ui.section == SettingsSection::NewProvider {
            self.settings_ui.section = SettingsSection::General;
        }
        self.settings_ui.provider_editor = None;
        self.settings_ui.form_error = None;
        cx.notify();
    }

    pub(crate) fn request_delete_provider(&mut self, id: String, cx: &mut Context<Self>) {
        self.overlays.destructive_action = Some(DestructiveAction::DeleteProvider { id });
        self.navigation.pending_focus = Some(PendingFocus::Root);
        self.overlays.command_palette_open = false;
        self.overlays.model_picker_open = false;
        cx.notify();
    }

    pub(super) fn delete_provider(&mut self, id: String, cx: &mut Context<Self>) {
        self.settings_ui.connection_tests.remove(&id);
        if self.settings_ui.section == SettingsSection::Provider(id.clone()) {
            self.settings_ui.section = SettingsSection::General;
            self.settings_ui.provider_editor = None;
            self.settings_ui.model_editor = None;
        }
        self.mutate_and_reload(move |storage| storage.delete_provider(&id), cx);
    }

    pub(crate) fn begin_add_model(&mut self, provider_id: String, cx: &mut Context<Self>) {
        let Some(provider_kind) = self
            .data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .map(|provider| provider.kind)
        else {
            self.settings_ui.form_error = Some("Provider not found.".into());
            cx.notify();
            return;
        };
        self.settings_ui.section = SettingsSection::Provider(provider_id.clone());
        self.settings_ui.provider_editor = None;
        self.install_model_editor(
            ModelEditor::new(provider_id.clone(), provider_kind, None, cx),
            cx,
        );
        self.settings_ui.form_error = None;
        self.fetch_available_models(provider_id, cx);
        cx.notify();
    }

    pub(crate) fn begin_edit_model(&mut self, id: String, cx: &mut Context<Self>) {
        let model = self
            .data
            .snapshot
            .models
            .iter()
            .find(|model| model.id == id)
            .cloned();
        if let Some(model) = model {
            let Some(provider_kind) = self
                .data
                .snapshot
                .providers
                .iter()
                .find(|provider| provider.id == model.provider_id)
                .map(|provider| provider.kind)
            else {
                self.settings_ui.form_error = Some("Provider not found.".into());
                cx.notify();
                return;
            };
            self.settings_ui.section = SettingsSection::Provider(model.provider_id.clone());
            self.settings_ui.provider_editor = None;
            let provider_id = model.provider_id.clone();
            self.install_model_editor(
                ModelEditor::new(provider_id.clone(), provider_kind, Some(model), cx),
                cx,
            );
            self.settings_ui.form_error = None;
            self.fetch_available_models(provider_id, cx);
            cx.notify();
        }
    }

    fn install_model_editor(&mut self, editor: ModelEditor, cx: &mut Context<Self>) {
        let remote_id = editor.remote_id.clone();
        self.settings_ui.model_editor = Some(editor);
        cx.subscribe(&remote_id, |this, _, event, cx| match event {
            ComposerEvent::Changed(remote_id) => {
                if let Some(editor) = &mut this.settings_ui.model_editor {
                    editor.remote_id_changed(remote_id.clone(), cx);
                    cx.notify();
                }
            }
            ComposerEvent::Navigate(direction) => {
                if let Some(editor) = &mut this.settings_ui.model_editor {
                    editor.navigate_models(*direction, cx);
                    cx.notify();
                }
            }
            ComposerEvent::Submit(_) => this.confirm_available_model(cx),
            ComposerEvent::Cancel => {
                if let Some(editor) = &mut this.settings_ui.model_editor {
                    editor.close_model_menu();
                    cx.notify();
                }
            }
        })
        .detach();
    }

    fn fetch_available_models(&mut self, provider_id: String, cx: &mut Context<Self>) {
        let Some(provider) = self
            .data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .cloned()
        else {
            self.settings_ui.form_error = Some("Provider not found.".into());
            cx.notify();
            return;
        };
        let Some(editor) = &mut self.settings_ui.model_editor else {
            return;
        };
        editor.begin_fetch();
        self.settings_ui.model_fetch_revision =
            self.settings_ui.model_fetch_revision.wrapping_add(1);
        let revision = self.settings_ui.model_fetch_revision;
        let (sender, receiver) = async_channel::bounded(1);
        self.services.runtime.spawn(async move {
            let result: Result<Vec<AvailableModel>, _> = providers::list_models(&provider).await;
            let _ = sender.send(result).await;
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                if this.settings_ui.model_fetch_revision != revision {
                    return;
                }
                let editing_id = this
                    .settings_ui
                    .model_editor
                    .as_ref()
                    .and_then(ModelEditor::editing_id)
                    .map(str::to_string);
                let configured = this
                    .data
                    .snapshot
                    .models
                    .iter()
                    .filter(|model| model.provider_id == provider_id)
                    .filter(|model| Some(model.id.as_str()) != editing_id.as_deref())
                    .map(|model| model.remote_id.clone())
                    .collect::<HashSet<_>>();
                let Some(editor) = this
                    .settings_ui
                    .model_editor
                    .as_mut()
                    .filter(|editor| editor.provider_id == provider_id)
                else {
                    return;
                };
                match result {
                    Ok(Ok(mut models)) => {
                        models.retain(|model| !configured.contains(&model.id));
                        editor.finish_fetch(models, cx);
                    }
                    Ok(Err(error)) => editor.fail_fetch(error.message),
                    Err(_) => editor.fail_fetch("Model discovery task stopped".into()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn retry_available_models(&mut self, cx: &mut Context<Self>) {
        if let Some(provider_id) = self
            .settings_ui
            .model_editor
            .as_ref()
            .map(|editor| editor.provider_id.clone())
        {
            self.fetch_available_models(provider_id, cx);
        }
    }

    pub(crate) fn toggle_available_model_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.toggle_model_menu();
            cx.notify();
        }
    }

    pub(crate) fn select_available_model(&mut self, remote_id: String, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.select_model(remote_id, cx);
            cx.notify();
        }
    }

    fn confirm_available_model(&mut self, cx: &mut Context<Self>) {
        let remote_id = self
            .settings_ui
            .model_editor
            .as_ref()
            .filter(|editor| editor.model_menu_open)
            .and_then(|editor| editor.selected_model_id(cx));
        if let Some(remote_id) = remote_id {
            self.select_available_model(remote_id, cx);
        }
    }

    pub(crate) fn toggle_model_capability(
        &mut self,
        capability: Capability,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.toggle_capability(capability);
            cx.notify();
        }
    }

    pub(crate) fn save_model(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.settings_ui.model_editor else {
            return;
        };
        let model = match editor.build(cx) {
            Ok(model) => model,
            Err(error) => {
                self.settings_ui.form_error = Some(error);
                cx.notify();
                return;
            }
        };
        let insert = editor.is_new();
        self.settings_ui.model_editor = None;
        self.settings_ui.form_error = None;
        self.mutate_and_reload(
            move |storage| {
                if insert {
                    storage.insert_model(&model)
                } else {
                    storage.update_model(&model)
                }
            },
            cx,
        );
    }

    pub(crate) fn cancel_model_editor(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.model_editor = None;
        self.settings_ui.form_error = None;
        cx.notify();
    }

    pub(crate) fn request_delete_model(&mut self, id: String, cx: &mut Context<Self>) {
        self.overlays.destructive_action = Some(DestructiveAction::DeleteModel { id });
        self.navigation.pending_focus = Some(PendingFocus::Root);
        self.overlays.command_palette_open = false;
        self.overlays.model_picker_open = false;
        cx.notify();
    }

    pub(super) fn delete_model(&mut self, id: String, cx: &mut Context<Self>) {
        self.mutate_and_reload(move |storage| storage.delete_model(&id), cx);
    }

    pub(crate) fn test_provider_connection(&mut self, provider_id: String, cx: &mut Context<Self>) {
        let Some(provider) = self
            .data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .cloned()
        else {
            return;
        };
        self.settings_ui
            .connection_tests
            .insert(provider_id.clone(), ConnectionTestStatus::Testing);
        let (sender, receiver) = async_channel::bounded(1);
        self.services.runtime.spawn(async move {
            let _ = sender
                .send(providers::test_connection(&provider).await)
                .await;
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                let status = match result {
                    Ok(Ok(())) => ConnectionTestStatus::Connected,
                    Ok(Err(error)) => ConnectionTestStatus::Failed(error.message),
                    Err(_) => ConnectionTestStatus::Failed("Connection task stopped".into()),
                };
                this.settings_ui
                    .connection_tests
                    .insert(provider_id, status);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
