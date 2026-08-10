use super::*;

impl OneChat {
    pub(crate) fn begin_add_prompt_variable(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_prompt_variable_edit(None, window, cx);
    }

    pub(crate) fn begin_edit_prompt_variable(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(source) = self
            .data
            .snapshot
            .settings
            .prompt_variables
            .get(&name)
            .cloned()
        else {
            return;
        };
        self.begin_prompt_variable_edit(Some((name, source)), window, cx);
    }

    fn begin_prompt_variable_edit(
        &mut self,
        variable: Option<(String, PromptVariableSource)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_ui.prompt_variable_test_revision = self
            .settings_ui
            .prompt_variable_test_revision
            .wrapping_add(1);
        self.settings_ui.prompt_variable_editor =
            Some(PromptVariableEditor::new(variable, window, cx));
        self.settings_ui.form_error = None;
        self.navigation.pending_focus = Some(PendingFocus::SettingsPrompt);
        let app = cx.entity();
        window.open_dialog(cx, move |dialog, window, cx| {
            crate::desktop::ui::settings::prompt_variable_dialog(dialog, app.clone(), window, cx)
        });
        cx.notify();
    }

    pub(crate) fn set_prompt_variable_kind(
        &mut self,
        kind: PromptVariableKind,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = self.settings_ui.prompt_variable_editor.as_mut() {
            editor.kind = kind;
            editor.test_status = None;
            self.settings_ui.form_error = None;
            cx.notify();
        }
    }

    pub(crate) fn toggle_prompt_variable_advanced(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = self.settings_ui.prompt_variable_editor.as_mut() {
            editor.advanced_expanded = !editor.advanced_expanded;
            cx.notify();
        }
    }

    pub(crate) fn toggle_prompt_builtins(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.prompt_builtins_expanded = !self.settings_ui.prompt_builtins_expanded;
        cx.notify();
    }

    pub(crate) fn test_prompt_variable_command(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.settings_ui.prompt_variable_editor.as_ref() else {
            return;
        };
        let source = match editor.source(cx) {
            Ok(PromptVariableSource::Command {
                script,
                cwd,
                timeout_ms,
            }) => PromptVariableSource::Command {
                script,
                cwd,
                timeout_ms,
            },
            Ok(_) => return,
            Err(error) => {
                self.settings_ui.form_error = Some(error);
                cx.notify();
                return;
            }
        };

        self.settings_ui.prompt_variable_test_revision = self
            .settings_ui
            .prompt_variable_test_revision
            .wrapping_add(1);
        let revision = self.settings_ui.prompt_variable_test_revision;
        if let Some(editor) = self.settings_ui.prompt_variable_editor.as_mut() {
            editor.test_status = Some(PromptVariableTestStatus::Running);
        }
        self.settings_ui.form_error = None;
        cx.notify();

        let variables = BTreeMap::from([("__test__".to_string(), source)]);
        let (sender, receiver) = async_channel::bounded(1);
        self.services.runtime.spawn(async move {
            let result = render_prompt(
                "{{__test__}}".into(),
                variables,
                PromptContext::default(),
                CancellationToken::new(),
            )
            .await;
            let _ = sender.send(result).await;
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                if this.settings_ui.prompt_variable_test_revision != revision {
                    return;
                }
                let Some(editor) = this.settings_ui.prompt_variable_editor.as_mut() else {
                    return;
                };
                editor.test_status = Some(match result {
                    Ok(Ok(snapshot)) => PromptVariableTestStatus::Succeeded {
                        duration_ms: snapshot
                            .variables
                            .first()
                            .map_or(0, |evaluation| evaluation.duration_ms),
                        output: snapshot.resolved,
                    },
                    Ok(Err(error)) => PromptVariableTestStatus::Failed(error.to_string()),
                    Err(_) => PromptVariableTestStatus::Failed(
                        "Command test task stopped unexpectedly.".into(),
                    ),
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn cancel_prompt_variable_edit(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.prompt_variable_test_revision = self
            .settings_ui
            .prompt_variable_test_revision
            .wrapping_add(1);
        self.settings_ui.prompt_variable_editor = None;
        self.settings_ui.form_error = None;
        cx.notify();
    }

    pub(crate) fn save_prompt_variable(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(editor) = self.settings_ui.prompt_variable_editor.as_ref() else {
            return false;
        };
        let (name, source) = match editor.build(cx) {
            Ok(variable) => variable,
            Err(error) => {
                self.settings_ui.form_error = Some(error);
                cx.notify();
                return false;
            }
        };
        let original_name = editor.original_name().map(str::to_string);
        if original_name.as_deref() != Some(&name)
            && self
                .data
                .snapshot
                .settings
                .prompt_variables
                .contains_key(&name)
        {
            self.settings_ui.form_error =
                Some(format!("A prompt variable named {name} already exists."));
            cx.notify();
            return false;
        }
        if let Some(original_name) = original_name {
            self.data
                .snapshot
                .settings
                .prompt_variables
                .remove(&original_name);
        }
        self.data
            .snapshot
            .settings
            .prompt_variables
            .insert(name, source);
        self.settings_ui.prompt_variable_editor = None;
        self.settings_ui.form_error = None;
        self.save_settings(cx);
        cx.notify();
        true
    }

    pub(crate) fn request_delete_prompt_variable(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_ui.prompt_variable_editor = None;
        self.request_destructive_action(
            DestructiveAction::DeletePromptVariable { name },
            window,
            cx,
        );
    }

    pub(crate) fn delete_prompt_variable(&mut self, name: String, cx: &mut Context<Self>) {
        if self
            .data
            .snapshot
            .settings
            .prompt_variables
            .remove(&name)
            .is_some()
        {
            self.save_settings(cx);
            cx.notify();
        }
    }
}
