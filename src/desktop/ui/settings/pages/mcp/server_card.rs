use super::*;

mod details;

use details::append_server_details;

pub(super) fn mcp_server_card(
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
        card = append_server_details(card, app, server, error, cx);
    }
    card.into_any_element()
}

pub(super) fn mcp_executable_row(name: &str, path: &str, available: bool, cx: &App) -> AnyElement {
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
