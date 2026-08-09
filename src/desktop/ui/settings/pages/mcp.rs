use super::super::*;

pub(in crate::desktop::ui::settings) fn mcp_page(
    app: &OneChat,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let snapshot = &app.mcp.snapshot;
    let actions = div()
        .flex_none()
        .flex()
        .items_center()
        .gap_2()
        .when(app.mcp.loading, |actions| {
            actions.child(Spinner::new().small().color(cx.theme().primary))
        })
        .child(
            icon_action(
                "open-mcp-config",
                AppIcon::Eye,
                IconTone::Muted,
                "Open MCP config",
                cx,
            )
            .on_click(cx.listener(|this, _, _, cx| this.open_mcp_config(cx))),
        )
        .child(
            icon_action(
                "reload-mcp-servers",
                AppIcon::Regenerate,
                IconTone::Muted,
                "Reload MCP servers",
                cx,
            )
            .disabled(app.mcp.loading)
            .on_click(cx.listener(|this, _, _, cx| this.reload_mcp(cx))),
        );

    let mut configuration = div().w_full().flex().flex_col().child(summary_row(
        "Config File",
        snapshot.config_path.display().to_string(),
        cx,
    ));
    if let Some(error) = &snapshot.config_error {
        configuration = configuration.child(
            div()
                .px_2()
                .pb_2()
                .child(Alert::error("mcp-config-error", error.clone())),
        );
    }
    if !snapshot.executables.is_empty() {
        configuration = configuration.child(setting_divider(cx));
    }
    for (index, executable) in snapshot.executables.iter().enumerate() {
        let detail = executable
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "Not found in PATH".to_string());
        configuration = configuration
            .child(mcp_executable_row(
                &executable.name,
                &detail,
                executable.path.is_some(),
                cx,
            ))
            .when(index + 1 < snapshot.executables.len(), |content| {
                content.child(setting_divider(cx))
            });
    }

    let servers = if snapshot.servers.is_empty() {
        div()
            .w_full()
            .px_4()
            .py_6()
            .text_center()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(if app.mcp.loading {
                "Loading MCP configuration…"
            } else {
                "No MCP servers configured"
            })
            .into_any_element()
    } else {
        stretching_column()
            .gap_2()
            .children(
                snapshot
                    .servers
                    .iter()
                    .map(|server| mcp_server_card(app, server, cx)),
            )
            .into_any_element()
    };
    let server_count = snapshot.servers.len();
    let server_label = format!(
        "{server_count} {}",
        if server_count == 1 {
            "server"
        } else {
            "servers"
        }
    );

    let server_actions = div()
        .flex_none()
        .flex()
        .items_center()
        .gap_2()
        .child(status_pill(server_label, false, cx))
        .child(
            primary_icon_action("add-mcp-server", AppIcon::Plus, "Add MCP server", cx)
                .disabled(app.mcp.loading)
                .on_click(cx.listener(|this, _, window, cx| this.begin_add_mcp_server(window, cx))),
        );

    let mut content = div().flex().flex_col().gap_6().child(page_header(
        "MCP Servers",
        "Connect to local or remote MCP tool servers.",
        cx,
    ));
    if let Some(error) = &app.settings_ui.mcp_error {
        content = content.child(Alert::error("mcp-editor-error", error.clone()));
    }
    content = content.child(section_with_actions(
        "Configuration",
        Some("UI changes update only the relevant JSONC fields and preserve existing comments."),
        Some(actions.into_any_element()),
        configuration,
        cx,
    ));
    if let Some(editor) = &app.settings_ui.mcp_server_editor {
        content = content.child(section(
            if editor.is_new() {
                "New Server"
            } else {
                "Edit Server"
            },
            Some(if editor.mode == McpServerEditorMode::Import {
                "Import one or more MCP servers from JSON or JSONC."
            } else {
                match editor.transport {
                    McpServerTransportEditor::Stdio => "Configure a local stdio MCP process.",
                    McpServerTransportEditor::Http => "Connect to a Streamable HTTP MCP endpoint.",
                }
            }),
            mcp_server_form(
                editor,
                &app.settings_ui.mcp_json_import,
                app.mcp.loading,
                cx,
            ),
            cx,
        ));
    }
    content = content.child(section_with_actions(
        "Servers",
        Some("Enabled servers stay connected for the lifetime of OneChat."),
        Some(server_actions.into_any_element()),
        servers,
        cx,
    ));

    detail_page(content)
}

fn mcp_server_card(
    app: &OneChat,
    server: &McpServerSnapshot,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let expanded = app.settings_ui.expanded_mcp_server_ids.contains(&server.id);
    let test_status = app.settings_ui.mcp_connection_tests.get(&server.id);
    let testing = matches!(test_status, Some(ConnectionTestStatus::Testing));
    let (status, accent, error) = match test_status {
        Some(ConnectionTestStatus::Testing) => ("Testing", true, None),
        Some(ConnectionTestStatus::Connected) => ("Test passed", true, None),
        Some(ConnectionTestStatus::Failed(error)) => ("Test failed", false, Some(error.as_str())),
        None => match &server.status {
            McpServerStatus::Disabled => ("Disabled", false, None),
            McpServerStatus::AuthorizationRequired => ("Sign in required", false, None),
            McpServerStatus::Ready => ("Ready", true, None),
            McpServerStatus::Failed(error) => ("Failed", false, Some(error.as_str())),
            McpServerStatus::Stopped => ("Stopped", false, None),
        },
    };
    let endpoint = match &server.transport {
        McpServerTransportSnapshot::Stdio {
            command,
            resolved_command,
        } => resolved_command
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| command.clone()),
        McpServerTransportSnapshot::Http { url } => url.clone(),
    };
    let tool_count = server.tools.len();
    let tool_label = format!(
        "{tool_count} {}",
        if tool_count == 1 { "tool" } else { "tools" }
    );
    let toggle_id = server.id.clone();
    let test_id = server.id.clone();
    let auth_id = server.id.clone();
    let edit_id = server.id.clone();
    let delete_id = server.id.clone();
    let expand_id = server.id.clone();

    let mut card = div()
        .w_full()
        .rounded(px(10.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .px_4()
        .py_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(server.id.clone()),
                        )
                        .child(
                            div()
                                .pt_0p5()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(11.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(endpoint),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(status_pill(tool_label, false, cx))
                                .child(status_pill(status, accent, cx))
                                .child(
                                    Switch::new(SharedString::from(format!(
                                        "toggle-mcp-server-{}",
                                        server.id
                                    )))
                                    .small()
                                    .checked(server.enabled)
                                    .color(cx.theme().primary)
                                    .disabled(app.mcp.loading)
                                    .tooltip(if server.enabled {
                                        "Disable MCP server"
                                    } else {
                                        "Enable MCP server"
                                    })
                                    .on_click(cx.listener(move |this, enabled: &bool, _, cx| {
                                        this.set_mcp_server_enabled(toggle_id.clone(), *enabled, cx)
                                    })),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_0()
                                .children(server.interactive_oauth.then(|| {
                                    icon_action(
                                        SharedString::from(format!(
                                            "authenticate-mcp-server-{}",
                                            server.id
                                        )),
                                        AppIcon::Key,
                                        IconTone::Muted,
                                        "Authenticate MCP server",
                                        cx,
                                    )
                                    .size(px(24.0))
                                    .loading(testing)
                                    .disabled(testing || app.mcp.loading)
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.authenticate_mcp_server(auth_id.clone(), cx)
                                        },
                                    ))
                                }))
                                .child(
                                    icon_action(
                                        SharedString::from(format!(
                                            "test-mcp-server-{}",
                                            server.id
                                        )),
                                        AppIcon::Plug,
                                        IconTone::Muted,
                                        "Test MCP server",
                                        cx,
                                    )
                                    .size(px(24.0))
                                    .loading(testing)
                                    .disabled(testing || app.mcp.loading)
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.test_mcp_server(test_id.clone(), cx)
                                        },
                                    )),
                                )
                                .child(
                                    icon_action(
                                        SharedString::from(format!(
                                            "edit-mcp-server-{}",
                                            server.id
                                        )),
                                        AppIcon::Pencil,
                                        IconTone::Muted,
                                        "Edit MCP server",
                                        cx,
                                    )
                                    .size(px(24.0))
                                    .disabled(app.mcp.loading)
                                    .on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            this.begin_edit_mcp_server(edit_id.clone(), window, cx)
                                        },
                                    )),
                                )
                                .child(
                                    icon_action(
                                        SharedString::from(format!(
                                            "delete-mcp-server-{}",
                                            server.id
                                        )),
                                        AppIcon::Trash,
                                        IconTone::Danger,
                                        "Delete MCP server",
                                        cx,
                                    )
                                    .size(px(24.0))
                                    .disabled(app.mcp.loading)
                                    .on_click(cx.listener(
                                        move |this, _, window, cx| {
                                            this.request_delete_mcp_server(
                                                delete_id.clone(),
                                                window,
                                                cx,
                                            )
                                        },
                                    )),
                                )
                                .child(
                                    icon_action(
                                        SharedString::from(format!(
                                            "expand-mcp-server-{}",
                                            server.id
                                        )),
                                        if expanded {
                                            AppIcon::ChevronUp
                                        } else {
                                            AppIcon::ChevronDown
                                        },
                                        IconTone::Muted,
                                        if expanded {
                                            "Collapse MCP server"
                                        } else {
                                            "Expand MCP server"
                                        },
                                        cx,
                                    )
                                    .size(px(24.0))
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.toggle_mcp_server_expanded(expand_id.clone(), cx)
                                        },
                                    )),
                                ),
                        ),
                ),
        );

    if expanded {
        if let Some(implementation) = &server.implementation {
            card = card.child(
                div()
                    .text_size(px(11.0))
                    .text_color(cx.theme().muted_foreground)
                    .child(implementation.clone()),
            );
        }
        if let Some(error) = error {
            card = card.child(
                div()
                    .text_size(px(12.0))
                    .line_height(px(18.0))
                    .text_color(cx.theme().danger)
                    .child(error.to_string()),
            );
        }
        if !server.tools.is_empty() {
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(server.tools.iter().map(|tool| {
                        let server_id = server.id.clone();
                        let tool_name = tool.name.clone();
                        let label = tool.title.as_deref().unwrap_or(&tool.name);
                        div()
                            .rounded(px(7.0))
                            .bg(cx.theme().popover)
                            .px_3()
                            .py_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(label.to_string()),
                                    )
                                    .child(
                                        Switch::new(SharedString::from(format!(
                                            "toggle-mcp-tool-{}-{}",
                                            server.id, tool.name
                                        )))
                                        .small()
                                        .checked(tool.enabled)
                                        .color(cx.theme().primary)
                                        .disabled(app.mcp.loading)
                                        .tooltip(if tool.enabled {
                                            "Disable MCP tool"
                                        } else {
                                            "Enable MCP tool"
                                        })
                                        .on_click(
                                            cx.listener(move |this, enabled: &bool, _, cx| {
                                                this.set_mcp_tool_enabled(
                                                    server_id.clone(),
                                                    tool_name.clone(),
                                                    *enabled,
                                                    cx,
                                                )
                                            }),
                                        ),
                                    ),
                            )
                            .children(tool.description.as_ref().map(|description| {
                                div()
                                    .pt_0p5()
                                    .text_size(px(11.0))
                                    .line_height(px(16.0))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(description.clone())
                            }))
                    })),
            );
        }
    }
    card.into_any_element()
}

fn mcp_executable_row(name: &str, path: &str, available: bool, cx: &App) -> AnyElement {
    div()
        .w_full()
        .min_h(px(44.0))
        .px_4()
        .py_2()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .w(px(64.0))
                .flex_none()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(name.to_string()),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(12.0))
                .text_color(cx.theme().muted_foreground)
                .child(path.to_string()),
        )
        .child(status_pill(
            if available { "Available" } else { "Missing" },
            available,
            cx,
        ))
        .into_any_element()
}
