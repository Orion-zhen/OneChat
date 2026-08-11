use super::*;

impl OneChat {
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
        self.spawn_tokio(
            async move { manager.upsert_server(id, server).await },
            cx,
            |this, result, cx| {
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
            },
        );
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
        self.spawn_tokio(
            async move { manager.import_servers(source).await },
            cx,
            |this, result, cx| {
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
            },
        );
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
        let delete_id = id.clone();
        self.spawn_tokio(
            async move { manager.delete_server(delete_id).await },
            cx,
            move |this, result, cx| {
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
            },
        );
    }
}
