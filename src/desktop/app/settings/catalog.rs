use std::collections::HashSet;

use gpui::{AppContext as _, Context, Entity, Task, Window};
use gpui_component::{
    combobox::ComboboxEvent,
    input::{InputEvent, InputState},
    select::SelectEvent,
};

use crate::{
    desktop::{
        app::{ConnectionTestStatus, DestructiveAction, OneChat, ProviderEditorExit},
        ui::settings::{
            Capability, KnownReasoningFormatItem, ModelEditor, ProviderEditor, ReasoningEditorMode,
            ReasoningParameterScope, SettingsSection,
        },
    },
    domain::{ReasoningLevel, now_timestamp},
    providers::{self, AvailableModel},
};

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
        self.request_provider_editor_exit(ProviderEditorExit::Section(destination), window, cx);
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

    pub(crate) fn begin_add_model(
        &mut self,
        provider_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
            ModelEditor::new(provider_id.clone(), provider_kind, None, window, cx),
            window,
            cx,
        );
        self.settings_ui.form_error = None;
        self.fetch_available_models(provider_id, cx);
        cx.notify();
    }

    pub(crate) fn begin_edit_model(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
                ModelEditor::new(provider_id.clone(), provider_kind, Some(model), window, cx),
                window,
                cx,
            );
            self.settings_ui.form_error = None;
            self.fetch_available_models(provider_id, cx);
            cx.notify();
        }
    }

    fn install_model_editor(
        &mut self,
        editor: ModelEditor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let remote_id = editor.remote_id.clone();
        let reasoning_format = editor.reasoning.format_select.clone();
        self.settings_ui.model_editor = Some(editor);
        cx.subscribe_in(
            &remote_id,
            window,
            |this,
             _,
             event: &ComboboxEvent<crate::desktop::ui::settings::ModelIdDelegate>,
             window,
             cx| {
                let ComboboxEvent::Change(values) = event else {
                    return;
                };
                let Some(remote_id) = values.last() else {
                    return;
                };
                if let Some(editor) = &mut this.settings_ui.model_editor {
                    editor.select_model(remote_id.clone(), window, cx);
                    cx.notify();
                }
            },
        )
        .detach();
        cx.subscribe_in(
            &reasoning_format,
            window,
            |this, _, event: &SelectEvent<Vec<KnownReasoningFormatItem>>, window, cx| {
                let SelectEvent::Confirm(Some(format)) = event else {
                    return;
                };
                if let Some(editor) = &mut this.settings_ui.model_editor {
                    editor.reasoning.set_format(*format, window, cx);
                    cx.notify();
                }
            },
        )
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

    pub(crate) fn set_model_capability(
        &mut self,
        capability: Capability,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.set_capability(capability, enabled);
            cx.notify();
        }
    }

    pub(crate) fn set_model_reasoning_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.set_enabled(enabled);
            cx.notify();
        }
    }

    pub(crate) fn set_model_reasoning_mode(
        &mut self,
        mode: ReasoningEditorMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.set_mode(mode, window, cx);
            cx.notify();
        }
    }

    pub(crate) fn toggle_known_reasoning_preset(
        &mut self,
        level: ReasoningLevel,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.toggle_known_preset(level, enabled);
            cx.notify();
        }
    }

    pub(crate) fn set_known_reasoning_default(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.set_known_default(id);
            cx.notify();
        }
    }

    pub(crate) fn add_custom_reasoning_preset(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.add_custom_preset(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn remove_custom_reasoning_preset(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.remove_custom_preset(index);
            cx.notify();
        }
    }

    pub(crate) fn move_custom_reasoning_preset(
        &mut self,
        index: usize,
        offset: isize,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.move_custom_preset(index, offset);
            cx.notify();
        }
    }

    pub(crate) fn set_custom_reasoning_default(
        &mut self,
        index: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.set_custom_default(index);
            cx.notify();
        }
    }

    pub(crate) fn add_reasoning_parameter(
        &mut self,
        preset: usize,
        scope: ReasoningParameterScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.add_parameter(preset, scope, window, cx);
            cx.notify();
        }
    }

    pub(crate) fn remove_reasoning_parameter(
        &mut self,
        preset: usize,
        scope: ReasoningParameterScope,
        parameter: usize,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.remove_parameter(preset, scope, parameter);
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

    pub(crate) fn request_delete_model(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_destructive_action(DestructiveAction::DeleteModel { id }, window, cx);
    }

    pub(crate) fn delete_model(&mut self, id: String, cx: &mut Context<Self>) {
        self.mutate_and_reload(move |storage| storage.delete_model(&id), cx);
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
                if let Some(editor) = &mut this.settings_ui.provider_editor {
                    editor.finish_test(revision, status);
                }
                cx.notify();
            });
        })
        .detach();
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
