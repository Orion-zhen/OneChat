use super::*;

impl OneChat {
    pub(crate) fn reload_mcp(&mut self, cx: &mut Context<Self>) {
        if self.mcp.loading {
            return;
        }
        self.mcp.loading = true;
        self.settings_ui.mcp_connection_tests.clear();
        cx.notify();

        let manager = self.services.mcp.clone();
        self.spawn_tokio(
            async move { manager.reload().await },
            cx,
            |this, result, cx| {
                this.mcp.loading = false;
                match result {
                    Ok(snapshot) => this.mcp.snapshot = snapshot,
                    Err(_) => {
                        this.settings_ui.mcp_error = Some("MCP reload task stopped".into());
                    }
                }
                cx.notify();
            },
        );
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
        let (url_sender, url_receiver) = async_channel::bounded::<String>(1);
        let auth_id = id.clone();
        cx.spawn(async move |_, cx| {
            if let Ok(url) = url_receiver.recv().await {
                cx.update(|cx| cx.open_url(&url));
            }
        })
        .detach();
        self.spawn_tokio(
            async move { manager.authorize_server(auth_id, url_sender).await },
            cx,
            move |this, result, cx| {
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
            },
        );
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
        let test_id = id.clone();
        self.spawn_tokio(
            async move { manager.test_server(test_id).await },
            cx,
            move |this, result, cx| {
                let status = match result {
                    Ok(Ok(())) => ConnectionTestStatus::Connected,
                    Ok(Err(error)) => ConnectionTestStatus::Failed(error.to_string()),
                    Err(_) => ConnectionTestStatus::Failed("MCP test task stopped".into()),
                };
                this.settings_ui.mcp_connection_tests.insert(id, status);
                cx.notify();
            },
        );
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
        let toggle_id = id.clone();
        self.spawn_tokio(
            async move { manager.set_server_enabled(toggle_id, enabled).await },
            cx,
            move |this, result, cx| {
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
            },
        );
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
        self.spawn_tokio(
            async move {
                manager
                    .set_tool_enabled(server_id, tool_name, enabled)
                    .await
            },
            cx,
            |this, result, cx| {
                this.mcp.loading = false;
                match result {
                    Ok(Ok(snapshot)) => this.mcp.snapshot = snapshot,
                    Ok(Err(error)) => this.settings_ui.mcp_error = Some(error.to_string()),
                    Err(_) => {
                        this.settings_ui.mcp_error = Some("MCP tool toggle task stopped".into());
                    }
                }
                cx.notify();
            },
        );
    }
}
