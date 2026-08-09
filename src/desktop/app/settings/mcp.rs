use gpui::{Context, Window};

use crate::{
    desktop::{
        app::{ConnectionTestStatus, DestructiveAction, OneChat},
        ui::settings::{McpServerEditor, McpServerEditorMode, McpServerTransportEditor},
    },
    mcp::{McpConfig, McpServerConfig},
};

impl OneChat {
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

    pub(crate) fn delete_mcp_server(&mut self, id: String, cx: &mut Context<Self>) {
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
}
