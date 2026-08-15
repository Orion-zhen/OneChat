use super::*;

impl OneChat {
    pub(crate) fn begin_add_provider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_ui.section = SettingsSection::NewProvider;
        self.install_provider_editor(ProviderEditor::new(None, window, cx), window, cx);
        self.settings_ui.pending_provider_exit = None;
        self.settings_ui.model_editor = None;
        self.settings_ui.form_error = None;
        if let Some(editor) = &self.settings_ui.provider_editor {
            editor.name.update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }

    pub(crate) fn begin_edit_provider(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let provider = self
            .data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == id)
            .cloned();
        if let Some(provider) = provider {
            self.settings_ui.section = SettingsSection::Provider(provider.id.clone());
            self.install_provider_editor(
                ProviderEditor::new(Some(provider), window, cx),
                window,
                cx,
            );
            self.settings_ui.pending_provider_exit = None;
            self.settings_ui.model_editor = None;
            self.settings_ui.form_error = None;
            cx.notify();
        }
    }

    fn install_provider_editor(
        &mut self,
        editor: ProviderEditor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let kind = editor.kind.clone();
        let inputs = provider_inputs(&editor);
        self.settings_ui.provider_editor = Some(editor);
        for input in inputs {
            self.subscribe_provider_input(input, window, cx);
        }
        cx.subscribe_in(
            &kind,
            window,
            |this,
             _,
             event: &SelectEvent<Vec<crate::desktop::ui::settings::ProviderKindItem>>,
             window,
             cx| {
                let SelectEvent::Confirm(Some(kind)) = event else {
                    return;
                };
                if let Some(editor) = &mut this.settings_ui.provider_editor {
                    editor.select_kind(*kind, window, cx);
                    cx.notify();
                }
            },
        )
        .detach();
    }

    fn subscribe_provider_input(
        &mut self,
        input: Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.subscribe_in(&input, window, |this, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                if let Some(editor) = &mut this.settings_ui.provider_editor {
                    editor.clear_feedback();
                }
                this.settings_ui.form_error = None;
                cx.notify();
            }
        })
        .detach();
    }

    pub(crate) fn set_provider_drop_target(
        &mut self,
        target: String,
        after: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        let target = match after {
            Some(after) => Some((target, after)),
            None if self
                .settings_ui
                .provider_drop_target
                .as_ref()
                .is_some_and(|(current, _)| current == &target) =>
            {
                None
            }
            None => return,
        };
        if self.settings_ui.provider_drop_target != target {
            self.settings_ui.provider_drop_target = target;
            cx.notify();
        }
    }

    pub(crate) fn reorder_provider(
        &mut self,
        provider_id: String,
        target_id: String,
        after: bool,
        cx: &mut Context<Self>,
    ) {
        self.settings_ui.provider_drop_target = None;
        let providers = &mut self.data.snapshot.providers;
        let Some(from) = providers
            .iter()
            .position(|provider| provider.id == provider_id)
        else {
            cx.notify();
            return;
        };
        let Some(target) = providers
            .iter()
            .position(|provider| provider.id == target_id)
        else {
            cx.notify();
            return;
        };

        let mut destination = target + usize::from(after);
        if from < destination {
            destination -= 1;
        }
        if from == destination {
            cx.notify();
            return;
        }

        let provider = providers.remove(from);
        providers.insert(destination.min(providers.len()), provider);
        let ordered_ids = providers
            .iter()
            .map(|provider| provider.id.clone())
            .collect::<Vec<_>>();
        self.mutate_and_reload(move |storage| storage.reorder_providers(&ordered_ids), cx);
        cx.notify();
    }

    pub(crate) fn set_provider_enabled(
        &mut self,
        id: String,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
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
        if provider.enabled == enabled {
            return;
        }
        provider.enabled = enabled;
        provider.updated_at = now_timestamp();
        self.mutate_and_reload(move |storage| storage.update_provider(&provider), cx);
    }

    pub(crate) fn set_provider_streaming(&mut self, streaming: bool, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.provider_editor {
            editor.streaming = streaming;
            editor.clear_feedback();
            self.settings_ui.form_error = None;
            cx.notify();
        }
    }

    pub(crate) fn add_provider_header(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let previous_len = self
            .settings_ui
            .provider_editor
            .as_ref()
            .map_or(0, |editor| editor.headers.len());
        if let Some(editor) = &mut self.settings_ui.provider_editor {
            editor.add_header(window, cx);
            editor.clear_feedback();
        }
        let inputs = self
            .settings_ui
            .provider_editor
            .as_ref()
            .filter(|editor| editor.headers.len() > previous_len)
            .and_then(|editor| editor.headers.last())
            .map(|header| [header.name.clone(), header.value.clone()]);
        if let Some(inputs) = inputs {
            for input in inputs {
                self.subscribe_provider_input(input, window, cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn remove_provider_header(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.provider_editor {
            editor.remove_header(index);
            editor.clear_feedback();
            self.settings_ui.form_error = None;
            cx.notify();
        }
    }

    pub(crate) fn save_provider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.settings_ui.provider_editor else {
            return;
        };
        if editor.saving
            || !editor.is_dirty(cx)
            || matches!(editor.test_status(cx), Some(ConnectionTestStatus::Testing))
        {
            return;
        }
        let provider = match editor.build(cx) {
            Ok(provider) => provider,
            Err(_) => {
                editor.focus_first_error(window, cx);
                self.settings_ui.form_error = None;
                cx.notify();
                return;
            }
        };
        let insert = editor.is_new();
        let provider_id = provider.id.clone();
        editor.saving = true;
        self.settings_ui.form_error = None;

        let previous = std::mem::replace(&mut self.data.storage_task, Task::ready(()));
        let storage = self.services.storage.clone();
        self.data.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move {
                    if insert {
                        storage.insert_provider(&provider)?;
                    } else {
                        storage.update_provider(&provider)?;
                    }
                    storage.load_snapshot()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(snapshot) => {
                        this.apply_snapshot(Ok(snapshot), cx);
                        this.settings_ui.pending_provider_exit = None;
                        this.settings_ui.provider_editor = None;
                        this.settings_ui.form_error = None;
                        this.settings_ui.section = SettingsSection::Provider(provider_id.clone());
                    }
                    Err(error) => {
                        if let Some(editor) = &mut this.settings_ui.provider_editor {
                            editor.saving = false;
                        }
                        this.settings_ui.form_error =
                            Some(format!("Could not save provider: {error}"));
                    }
                }
                cx.notify();
            });
        });
        cx.notify();
    }

    pub(crate) fn cancel_provider_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let destination = if self.settings_ui.section == SettingsSection::NewProvider {
            SettingsSection::General
        } else {
            self.settings_ui.section.clone()
        };
        self.request_provider_editor_exit(SettingsDestination::Section(destination), window, cx);
    }

    pub(crate) fn request_delete_provider(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_destructive_action(DestructiveAction::DeleteProvider { id }, window, cx);
    }

    pub(crate) fn delete_provider(&mut self, id: String, cx: &mut Context<Self>) {
        self.settings_ui.connection_tests.remove(&id);
        if self.settings_ui.section == SettingsSection::Provider(id.clone()) {
            self.settings_ui.section = SettingsSection::General;
            self.settings_ui.provider_editor = None;
            self.settings_ui.model_editor = None;
        }
        self.mutate_and_reload(move |storage| storage.delete_provider(&id), cx);
    }

    pub(crate) fn test_provider_editor_connection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = &mut self.settings_ui.provider_editor else {
            return;
        };
        if editor.saving || matches!(editor.test_status(cx), Some(ConnectionTestStatus::Testing)) {
            return;
        }
        let (provider, revision) = match editor.begin_test(cx) {
            Ok(test) => test,
            Err(_) => {
                editor.focus_first_error(window, cx);
                self.settings_ui.form_error = None;
                cx.notify();
                return;
            }
        };
        self.settings_ui.form_error = None;
        self.spawn_tokio(
            async move { providers::test_connection(&provider).await },
            cx,
            move |this, result, cx| {
                let status = match result {
                    Ok(Ok(())) => ConnectionTestStatus::Connected,
                    Ok(Err(error)) => ConnectionTestStatus::Failed(error.message),
                    Err(_) => ConnectionTestStatus::Failed("Connection task stopped".into()),
                };
                if let Some(editor) = &mut this.settings_ui.provider_editor {
                    editor.finish_test(revision, status);
                }
                cx.notify();
            },
        );
        cx.notify();
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
        self.spawn_tokio(
            async move { providers::test_connection(&provider).await },
            cx,
            move |this, result, cx| {
                let status = match result {
                    Ok(Ok(())) => ConnectionTestStatus::Connected,
                    Ok(Err(error)) => ConnectionTestStatus::Failed(error.message),
                    Err(_) => ConnectionTestStatus::Failed("Connection task stopped".into()),
                };
                this.settings_ui
                    .connection_tests
                    .insert(provider_id, status);
                cx.notify();
            },
        );
        cx.notify();
    }
}

fn provider_inputs(editor: &ProviderEditor) -> Vec<Entity<InputState>> {
    let mut inputs = vec![
        editor.name.clone(),
        editor.endpoint.clone(),
        editor.api_key.clone(),
        editor.proxy.clone(),
    ];
    for header in &editor.headers {
        inputs.push(header.name.clone());
        inputs.push(header.value.clone());
    }
    inputs
}
