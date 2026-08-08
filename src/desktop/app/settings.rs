use super::*;

impl OneChat {
    fn font_families(&self, role: FontRole) -> &[String] {
        match role {
            FontRole::Ui => &self.data.snapshot.settings.ui_font_families,
            FontRole::Code => &self.data.snapshot.settings.code_font_families,
        }
    }

    fn set_font_families(&mut self, role: FontRole, families: Vec<String>, cx: &mut Context<Self>) {
        let families = crate::domain::normalize_font_families(families, role.default_family());
        let stored = match role {
            FontRole::Ui => &mut self.data.snapshot.settings.ui_font_families,
            FontRole::Code => &mut self.data.snapshot.settings.code_font_families,
        };
        if *stored == families {
            return;
        }
        *stored = families;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn add_font_family(
        &mut self,
        role: FontRole,
        family: String,
        cx: &mut Context<Self>,
    ) {
        let mut families = self.font_families(role).to_vec();
        if families.iter().any(|item| item == &family) {
            return;
        }
        families.push(family);
        self.set_font_families(role, families, cx);
    }

    pub(crate) fn move_font_family(
        &mut self,
        role: FontRole,
        index: usize,
        up: bool,
        cx: &mut Context<Self>,
    ) {
        let mut families = self.font_families(role).to_vec();
        let Some(target) = (if up {
            index.checked_sub(1)
        } else {
            index
                .checked_add(1)
                .filter(|target| *target < families.len())
        }) else {
            return;
        };
        families.swap(index, target);
        self.set_font_families(role, families, cx);
    }

    pub(crate) fn remove_font_family(
        &mut self,
        role: FontRole,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        let mut families = self.font_families(role).to_vec();
        if families.len() <= 1 || index >= families.len() {
            return;
        }
        families.remove(index);
        self.set_font_families(role, families, cx);
    }

    pub(crate) fn update_background_opacity(&mut self, opacity: f32, cx: &mut Context<Self>) {
        let opacity = rounded_background_opacity(opacity);
        if (self.data.snapshot.settings.background_opacity - opacity).abs() < f32::EPSILON {
            return;
        }
        self.data.snapshot.settings.background_opacity = opacity;
        cx.notify();
    }

    pub(crate) fn update_message_width_ratio(&mut self, ratio: f32, cx: &mut Context<Self>) {
        let ratio = rounded_message_width_ratio(ratio);
        if (self.data.snapshot.settings.message_width_ratio - ratio).abs() < f32::EPSILON {
            return;
        }
        self.data.snapshot.settings.message_width_ratio = ratio;
        cx.notify();
    }

    pub(crate) fn update_message_font_size(&mut self, size: f32, cx: &mut Context<Self>) {
        let size = rounded_message_font_size(size);
        if (self.data.snapshot.settings.message_font_size - size).abs() < f32::EPSILON {
            return;
        }
        self.data.snapshot.settings.message_font_size = size;
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
        let reload_prompts = section == SettingsSection::SystemPrompts;
        if self.settings_ui.section == section {
            if reload_prompts {
                self.reload_snapshot(cx);
            }
            return;
        }
        self.settings_ui.section = section;
        self.settings_ui.viewed_prompt_preset = None;
        self.settings_ui.provider_editor = None;
        self.settings_ui.model_editor = None;
        self.settings_ui.prompt_preset_editor = None;
        self.settings_ui.title_prompt_editor = None;
        self.settings_ui.mcp_server_editor = None;
        self.settings_ui.mcp_error = None;
        self.settings_ui.form_error = None;
        if reload_prompts {
            self.reload_snapshot(cx);
        }
        cx.notify();
    }

    pub(crate) fn reload_mcp(&mut self, cx: &mut Context<Self>) {
        if self.mcp.loading {
            return;
        }
        self.mcp.loading = true;
        self.settings_ui.mcp_connection_tests.clear();
        cx.notify();

        let manager = self.services.mcp.clone();
        let (sender, receiver) = async_channel::bounded(1);
        self.services.runtime.spawn(async move {
            let snapshot = manager.reload().await;
            let _ = sender.send(snapshot).await;
        });
        cx.spawn(async move |this, cx| {
            let Ok(snapshot) = receiver.recv().await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.mcp.snapshot = snapshot;
                this.mcp.loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn open_mcp_config(&mut self, cx: &mut Context<Self>) {
        let path = self.mcp.snapshot.config_path.clone();
        if let Err(error) = std::process::Command::new("open").arg(&path).spawn() {
            self.data.error = Some(format!(
                "Could not open MCP config {}: {error}",
                path.display()
            ));
            cx.notify();
        }
    }

    pub(crate) fn toggle_mcp_server_expanded(&mut self, id: String, cx: &mut Context<Self>) {
        if !self.settings_ui.expanded_mcp_server_ids.remove(&id) {
            self.settings_ui.expanded_mcp_server_ids.insert(id);
        }
        cx.notify();
    }

    pub(crate) fn authenticate_mcp_server(&mut self, id: String, cx: &mut Context<Self>) {
        if matches!(
            self.settings_ui.mcp_connection_tests.get(&id),
            Some(ConnectionTestStatus::Testing)
        ) {
            return;
        }
        self.settings_ui
            .mcp_connection_tests
            .insert(id.clone(), ConnectionTestStatus::Testing);
        cx.notify();

        let manager = self.services.mcp.clone();
        let (url_sender, url_receiver) = async_channel::bounded(1);
        let (result_sender, result_receiver) = async_channel::bounded(1);
        let auth_id = id.clone();
        self.services.runtime.spawn(async move {
            let result = manager.authorize_server(auth_id, url_sender).await;
            let _ = result_sender.send(result).await;
        });
        cx.spawn(async move |_, cx| {
            if let Ok(url) = url_receiver.recv().await {
                cx.update(|cx| cx.open_url(&url));
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let result = result_receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                let status = match result {
                    Ok(Ok(snapshot)) => {
                        this.mcp.snapshot = snapshot;
                        ConnectionTestStatus::Connected
                    }
                    Ok(Err(error)) => ConnectionTestStatus::Failed(error.to_string()),
                    Err(_) => ConnectionTestStatus::Failed("MCP OAuth task stopped".into()),
                };
                this.settings_ui.mcp_connection_tests.insert(id, status);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn test_mcp_server(&mut self, id: String, cx: &mut Context<Self>) {
        if matches!(
            self.settings_ui.mcp_connection_tests.get(&id),
            Some(ConnectionTestStatus::Testing)
        ) {
            return;
        }
        self.settings_ui
            .mcp_connection_tests
            .insert(id.clone(), ConnectionTestStatus::Testing);
        cx.notify();

        let manager = self.services.mcp.clone();
        let (sender, receiver) = async_channel::bounded(1);
        let test_id = id.clone();
        self.services.runtime.spawn(async move {
            let _ = sender.send(manager.test_server(test_id).await).await;
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                let status = match result {
                    Ok(Ok(())) => ConnectionTestStatus::Connected,
                    Ok(Err(error)) => ConnectionTestStatus::Failed(error.to_string()),
                    Err(_) => ConnectionTestStatus::Failed("MCP test task stopped".into()),
                };
                this.settings_ui.mcp_connection_tests.insert(id, status);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn set_mcp_server_enabled(
        &mut self,
        id: String,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if self.mcp.loading {
            return;
        }
        self.mcp.loading = true;
        self.settings_ui.mcp_error = None;
        cx.notify();

        let manager = self.services.mcp.clone();
        let (sender, receiver) = async_channel::bounded(1);
        let toggle_id = id.clone();
        self.services.runtime.spawn(async move {
            let _ = sender
                .send(manager.set_server_enabled(toggle_id, enabled).await)
                .await;
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                this.mcp.loading = false;
                match result {
                    Ok(Ok(snapshot)) => {
                        this.mcp.snapshot = snapshot;
                        this.settings_ui.mcp_connection_tests.remove(&id);
                    }
                    Ok(Err(error)) => this.settings_ui.mcp_error = Some(error.to_string()),
                    Err(_) => {
                        this.settings_ui.mcp_error = Some("MCP toggle task stopped".into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn set_mcp_tool_enabled(
        &mut self,
        server_id: String,
        tool_name: String,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if self.mcp.loading {
            return;
        }
        self.mcp.loading = true;
        self.settings_ui.mcp_error = None;
        cx.notify();

        let manager = self.services.mcp.clone();
        let (sender, receiver) = async_channel::bounded(1);
        self.services.runtime.spawn(async move {
            let _ = sender
                .send(
                    manager
                        .set_tool_enabled(server_id, tool_name, enabled)
                        .await,
                )
                .await;
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                this.mcp.loading = false;
                match result {
                    Ok(Ok(snapshot)) => this.mcp.snapshot = snapshot,
                    Ok(Err(error)) => this.settings_ui.mcp_error = Some(error.to_string()),
                    Err(_) => {
                        this.settings_ui.mcp_error = Some("MCP tool toggle task stopped".into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn begin_add_mcp_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_ui.mcp_json_import.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.settings_ui.mcp_server_editor = Some(McpServerEditor::new(
            None,
            McpServerConfig::default(),
            window,
            cx,
        ));
        self.settings_ui.mcp_error = None;
        cx.notify();
    }

    pub(crate) fn begin_edit_mcp_server(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = McpConfig::load(&self.mcp.snapshot.config_path).and_then(|config| {
            config
                .servers
                .get(&id)
                .cloned()
                .ok_or_else(|| crate::mcp::McpError::new(format!("MCP server not found: {id}")))
        });
        match result {
            Ok(server) => {
                self.settings_ui.mcp_server_editor =
                    Some(McpServerEditor::new(Some(id), server, window, cx));
                self.settings_ui.mcp_error = None;
            }
            Err(error) => self.settings_ui.mcp_error = Some(error.to_string()),
        }
        cx.notify();
    }

    pub(crate) fn cancel_mcp_server_editor(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.mcp_server_editor = None;
        self.settings_ui.mcp_error = None;
        cx.notify();
    }

    pub(crate) fn select_mcp_server_editor_mode(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.mcp_server_editor
            && editor.is_new()
        {
            editor.mode = McpServerEditorMode::from_index(index);
            self.settings_ui.mcp_error = None;
            cx.notify();
        }
    }

    pub(crate) fn select_mcp_server_transport(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.mcp_server_editor {
            editor.transport = McpServerTransportEditor::from_index(index);
            self.settings_ui.mcp_error = None;
            cx.notify();
        }
    }

    pub(crate) fn add_mcp_argument(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.mcp_server_editor {
            editor.add_argument(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn remove_mcp_argument(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.mcp_server_editor {
            editor.remove_argument(index);
            cx.notify();
        }
    }

    pub(crate) fn add_mcp_environment_variable(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.mcp_server_editor {
            editor.add_environment_variable(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn remove_mcp_environment_variable(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.mcp_server_editor {
            editor.remove_environment_variable(index);
            cx.notify();
        }
    }

    pub(crate) fn add_mcp_header(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.mcp_server_editor {
            editor.add_header(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn remove_mcp_header(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.mcp_server_editor {
            editor.remove_header(index);
            cx.notify();
        }
    }

    pub(crate) fn select_mcp_oauth_mode(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.mcp_server_editor {
            editor.select_oauth_mode(index);
            self.settings_ui.mcp_error = None;
            cx.notify();
        }
    }

    pub(crate) fn save_mcp_server(&mut self, cx: &mut Context<Self>) {
        if self.mcp.loading {
            return;
        }
        let Some(editor) = &self.settings_ui.mcp_server_editor else {
            return;
        };
        let (id, server) = match editor.build(cx) {
            Ok(value) => value,
            Err(error) => {
                self.settings_ui.mcp_error = Some(error);
                cx.notify();
                return;
            }
        };
        if editor.is_new()
            && McpConfig::load(&self.mcp.snapshot.config_path)
                .is_ok_and(|config| config.servers.contains_key(&id))
        {
            self.settings_ui.mcp_error = Some(format!("An MCP server named {id} already exists."));
            cx.notify();
            return;
        }
        self.mcp.loading = true;
        self.settings_ui.mcp_error = None;
        cx.notify();

        let manager = self.services.mcp.clone();
        let (sender, receiver) = async_channel::bounded(1);
        self.services.runtime.spawn(async move {
            let _ = sender.send(manager.upsert_server(id, server).await).await;
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                this.mcp.loading = false;
                match result {
                    Ok(Ok(snapshot)) => {
                        this.mcp.snapshot = snapshot;
                        this.settings_ui.mcp_server_editor = None;
                        this.settings_ui.mcp_error = None;
                    }
                    Ok(Err(error)) => this.settings_ui.mcp_error = Some(error.to_string()),
                    Err(_) => {
                        this.settings_ui.mcp_error = Some("MCP save task stopped".into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn import_mcp_servers(&mut self, cx: &mut Context<Self>) {
        if self.mcp.loading {
            return;
        }
        let source = self
            .settings_ui
            .mcp_json_import
            .read(cx)
            .value()
            .to_string();
        if source.trim().is_empty() {
            self.settings_ui.mcp_error = Some("Paste an MCP JSON configuration first.".into());
            cx.notify();
            return;
        }
        self.mcp.loading = true;
        self.settings_ui.mcp_error = None;
        cx.notify();

        let manager = self.services.mcp.clone();
        let (sender, receiver) = async_channel::bounded(1);
        self.services.runtime.spawn(async move {
            let _ = sender.send(manager.import_servers(source).await).await;
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                this.mcp.loading = false;
                match result {
                    Ok(Ok((_, snapshot))) => {
                        this.mcp.snapshot = snapshot;
                        this.settings_ui.mcp_server_editor = None;
                        this.settings_ui.mcp_error = None;
                    }
                    Ok(Err(error)) => this.settings_ui.mcp_error = Some(error.to_string()),
                    Err(_) => {
                        this.settings_ui.mcp_error = Some("MCP import task stopped".into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn request_delete_mcp_server(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_destructive_action(DestructiveAction::DeleteMcpServer { id }, window, cx);
    }

    pub(super) fn delete_mcp_server(&mut self, id: String, cx: &mut Context<Self>) {
        if self.mcp.loading {
            return;
        }
        self.mcp.loading = true;
        self.settings_ui.mcp_error = None;
        cx.notify();

        let manager = self.services.mcp.clone();
        let (sender, receiver) = async_channel::bounded(1);
        let delete_id = id.clone();
        self.services.runtime.spawn(async move {
            let _ = sender.send(manager.delete_server(delete_id).await).await;
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                this.mcp.loading = false;
                match result {
                    Ok(Ok(snapshot)) => {
                        this.mcp.snapshot = snapshot;
                        this.settings_ui.mcp_server_editor = None;
                        this.settings_ui.mcp_error = None;
                        this.settings_ui.expanded_mcp_server_ids.remove(&id);
                        this.settings_ui.mcp_connection_tests.remove(&id);
                    }
                    Ok(Err(error)) => this.settings_ui.mcp_error = Some(error.to_string()),
                    Err(_) => {
                        this.settings_ui.mcp_error = Some("MCP delete task stopped".into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
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

    pub(crate) fn select_default_prompt(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        if name
            .as_deref()
            .is_some_and(|name| self.prompt_preset(name).is_none())
        {
            return;
        }
        if self.data.snapshot.settings.default_system_prompt_preset == name {
            cx.notify();
            return;
        }
        self.data.snapshot.settings.default_system_prompt_preset = name;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn view_prompt_preset(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.prompt_preset(&name).is_none() {
            return;
        }
        self.settings_ui.viewed_prompt_preset = Some(name);
        self.open_prompt_preset_dialog(window, cx);
        cx.notify();
    }

    pub(crate) fn begin_add_prompt_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_prompt_preset_edit(None, window, cx);
    }

    pub(crate) fn begin_edit_prompt_preset(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(preset) = self.prompt_preset(&name).cloned() else {
            return;
        };
        self.begin_prompt_preset_edit(Some(preset), window, cx);
    }

    fn begin_prompt_preset_edit(
        &mut self,
        preset: Option<SystemPromptPreset>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editor = PromptPresetEditor::new(preset, window, cx);
        self.settings_ui.viewed_prompt_preset = None;
        self.settings_ui.prompt_preset_editor = Some(editor);
        self.settings_ui.form_error = None;
        self.navigation.pending_focus = Some(PendingFocus::SettingsPrompt);
        self.open_prompt_preset_dialog(window, cx);
        cx.notify();
    }

    fn open_prompt_preset_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.chat.text_selection.clear(window);
        let app = cx.entity();
        window.open_dialog(cx, move |dialog, window, cx| {
            crate::desktop::ui::settings::prompt_preset_dialog(dialog, app.clone(), window, cx)
        });
    }

    pub(crate) fn cancel_prompt_preset_edit(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.prompt_preset_editor = None;
        self.settings_ui.form_error = None;
        cx.notify();
    }

    pub(crate) fn save_prompt_preset(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(editor) = self.settings_ui.prompt_preset_editor.as_ref() else {
            return false;
        };
        let preset = match editor.build(cx) {
            Ok(preset) => preset,
            Err(error) => {
                self.settings_ui.form_error = Some(error);
                cx.notify();
                return false;
            }
        };
        let original_name = editor.original_name().map(str::to_string);
        if original_name.as_deref() != Some(preset.name.as_str())
            && self.prompt_preset(&preset.name).is_some()
        {
            self.settings_ui.form_error = Some(format!(
                "A prompt preset named {} already exists.",
                preset.name
            ));
            cx.notify();
            return false;
        }
        let mut settings = self.data.snapshot.settings.clone();
        if let Some(original_name) = original_name.as_deref()
            && settings.default_system_prompt_preset.as_deref() == Some(original_name)
        {
            settings.default_system_prompt_preset = Some(preset.name.clone());
        }
        self.data.snapshot.settings = settings.clone();
        self.settings_ui.prompt_preset_editor = None;
        self.settings_ui.form_error = None;
        self.mutate_and_reload(
            move |storage| {
                if let Some(original_name) = original_name {
                    storage.update_prompt_preset(&original_name, &preset)?;
                } else {
                    storage.insert_prompt_preset(&preset)?;
                }
                storage.save_settings(&settings)
            },
            cx,
        );
        true
    }

    pub(crate) fn request_delete_prompt_preset(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_destructive_action(DestructiveAction::DeletePromptPreset { name }, window, cx);
    }

    pub(crate) fn delete_prompt_preset(&mut self, name: String, cx: &mut Context<Self>) {
        let mut settings = self.data.snapshot.settings.clone();
        if settings.default_system_prompt_preset.as_deref() == Some(&name) {
            settings.default_system_prompt_preset = None;
        }
        self.data.snapshot.settings = settings.clone();
        self.settings_ui.viewed_prompt_preset = None;
        self.settings_ui.prompt_preset_editor = None;
        self.mutate_and_reload(
            move |storage| {
                storage.delete_prompt_preset(&name)?;
                storage.save_settings(&settings)
            },
            cx,
        );
    }

    pub(crate) fn reload_prompt_presets(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.viewed_prompt_preset = None;
        self.reload_snapshot(cx);
    }

    pub(crate) fn begin_edit_title_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self
            .data
            .snapshot
            .settings
            .title_generation_system_prompt
            .clone();
        self.settings_ui.title_prompt_editor = Some(cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .soft_wrap(true)
                .default_value(value)
                .placeholder("Used to generate automatic conversation titles")
        }));
        self.navigation.pending_focus = Some(PendingFocus::SettingsPrompt);
        cx.notify();
    }

    pub(crate) fn cancel_title_prompt_edit(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.title_prompt_editor = None;
        cx.notify();
    }

    pub(crate) fn save_title_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.settings_ui.title_prompt_editor.as_ref() else {
            return;
        };
        let content = editor.read(cx).value().trim().to_string();
        if content.is_empty() {
            self.data.error = Some("The title generation prompt cannot be empty.".into());
            cx.notify();
            return;
        }
        self.data.snapshot.settings.title_generation_system_prompt = content;
        self.settings_ui.title_prompt_editor = None;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn reset_title_generation_prompt(&mut self, cx: &mut Context<Self>) {
        self.data.snapshot.settings.title_generation_system_prompt =
            DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT.into();
        self.settings_ui.title_prompt_editor = None;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn begin_add_provider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_ui.section = SettingsSection::NewProvider;
        self.install_provider_editor(ProviderEditor::new(None, window, cx), window, cx);
        self.settings_ui.model_editor = None;
        self.settings_ui.form_error = None;
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
        self.settings_ui.provider_editor = Some(editor);
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

    pub(crate) fn request_delete_provider(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_destructive_action(DestructiveAction::DeleteProvider { id }, window, cx);
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

fn rounded_background_opacity(opacity: f32) -> f32 {
    let opacity = opacity.clamp(
        crate::domain::MIN_BACKGROUND_OPACITY,
        crate::domain::MAX_BACKGROUND_OPACITY,
    );
    (opacity * 100.0).round() / 100.0
}

fn rounded_message_width_ratio(ratio: f32) -> f32 {
    let ratio = ratio.clamp(
        crate::domain::MIN_MESSAGE_WIDTH_RATIO,
        crate::domain::MAX_MESSAGE_WIDTH_RATIO,
    );
    (ratio * 100.0).round() / 100.0
}

fn rounded_message_font_size(size: f32) -> f32 {
    size.clamp(
        crate::domain::MIN_MESSAGE_FONT_SIZE,
        crate::domain::MAX_MESSAGE_FONT_SIZE,
    )
    .round()
}

#[cfg(test)]
mod tests {
    use super::{
        rounded_background_opacity, rounded_message_font_size, rounded_message_width_ratio,
    };

    #[test]
    fn background_opacity_is_clamped_and_rounded_to_slider_step() {
        assert_eq!(rounded_background_opacity(0.734), 0.73);
        assert_eq!(rounded_background_opacity(0.736), 0.74);
        assert_eq!(rounded_background_opacity(-0.1), 0.0);
        assert_eq!(rounded_background_opacity(1.1), 1.0);
    }

    #[test]
    fn message_width_ratio_is_clamped_and_rounded_to_slider_step() {
        assert_eq!(rounded_message_width_ratio(0.734), 0.73);
        assert_eq!(rounded_message_width_ratio(0.736), 0.74);
        assert_eq!(rounded_message_width_ratio(0.1), 0.5);
        assert_eq!(rounded_message_width_ratio(1.5), 1.0);
    }

    #[test]
    fn message_font_size_is_clamped_and_rounded_to_whole_pixels() {
        assert_eq!(rounded_message_font_size(14.4), 14.0);
        assert_eq!(rounded_message_font_size(14.6), 15.0);
        assert_eq!(rounded_message_font_size(5.0), 13.0);
        assert_eq!(rounded_message_font_size(30.0), 22.0);
    }
}
