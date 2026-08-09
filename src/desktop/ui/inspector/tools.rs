use super::*;

fn tool_status_pill(label: impl Into<SharedString>, accent: bool, cx: &App) -> AnyElement {
    div()
        .flex_none()
        .rounded_full()
        .bg(if accent {
            cx.theme().accent
        } else {
            cx.theme().background
        })
        .px_2()
        .py_1()
        .text_size(px(10.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if accent {
            cx.theme().primary
        } else {
            cx.theme().muted_foreground
        })
        .child(label.into())
        .into_any_element()
}

pub(super) fn render_tools(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let Some(conversation) = app.current_conversation() else {
        return notice("Select a conversation to configure its tools.", cx);
    };
    let generating = app.is_current_generating();
    let model_supports_tools = app
        .current_model()
        .is_some_and(|model| model.capabilities.tools);
    let available_count = app
        .mcp
        .snapshot
        .servers
        .iter()
        .filter(|server| server.enabled && server.status == McpServerStatus::Ready)
        .flat_map(|server| server.tools.iter())
        .count();
    let selected_count = app
        .mcp
        .snapshot
        .servers
        .iter()
        .filter(|server| server.enabled && server.status == McpServerStatus::Ready)
        .flat_map(|server| {
            server.tools.iter().filter(move |tool| {
                conversation
                    .tool_selection
                    .resolves(&server.id, &tool.name, tool.enabled)
            })
        })
        .count();
    let summary = match &conversation.tool_selection {
        ToolSelection::Default => format!(
            "{selected_count} of {available_count} available tools use the global defaults."
        ),
        ToolSelection::Only(_) => {
            format!("{selected_count} of {available_count} available tools are enabled.")
        }
    };

    let mut content =
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(cx.theme().muted_foreground)
                    .child(summary),
            )
            .when(!model_supports_tools, |content| {
                content.child(notice("The current model does not support tools.", cx))
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("enable-all-conversation-tools")
                            .small()
                            .compact()
                            .label("Enable all")
                            .disabled(generating)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_all_conversation_tools(true, cx)
                            })),
                    )
                    .child(
                        Button::new("reset-conversation-tools")
                            .small()
                            .compact()
                            .label("Use defaults")
                            .disabled(generating)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.reset_conversation_tool_selection(cx)
                            })),
                    )
                    .child(
                        Button::new("disable-all-conversation-tools")
                            .small()
                            .compact()
                            .label("Disable all")
                            .disabled(generating)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_all_conversation_tools(false, cx)
                            })),
                    ),
            );

    let mut has_tools = false;
    for server in &app.mcp.snapshot.servers {
        if server.tools.is_empty() {
            continue;
        }
        has_tools = true;
        let expanded = app
            .chat
            .expanded_conversation_tool_server_ids
            .contains(&server.id);
        let server_available = server.enabled && server.status == McpServerStatus::Ready;
        let enabled_count = server
            .tools
            .iter()
            .filter(|tool| {
                conversation
                    .tool_selection
                    .resolves(&server.id, &tool.name, tool.enabled)
            })
            .count();
        let all_enabled = server_available && enabled_count == server.tools.len();
        let tool_count = server.tools.len();
        let tool_label = format!(
            "{tool_count} {}",
            if tool_count == 1 { "tool" } else { "tools" }
        );
        let status = if server_available {
            format!("{enabled_count} enabled")
        } else {
            "Unavailable".to_string()
        };
        let toggle_id = server.id.clone();
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
                    .gap_3()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(server.id.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(tool_status_pill(tool_label, false, cx))
                            .child(tool_status_pill(status, all_enabled, cx))
                            .child(
                                Switch::new(SharedString::from(format!(
                                    "conversation-server-tools-{}",
                                    server.id
                                )))
                                .small()
                                .checked(all_enabled)
                                .color(cx.theme().primary)
                                .disabled(generating || !server_available || app.mcp.loading)
                                .tooltip(if all_enabled {
                                    "Disable this server's tools for the conversation"
                                } else {
                                    "Enable this server's tools for the conversation"
                                })
                                .on_click(cx.listener(
                                    move |this, enabled: &bool, _, cx| {
                                        this.set_conversation_server_tools_enabled(
                                            toggle_id.clone(),
                                            *enabled,
                                            cx,
                                        )
                                    },
                                )),
                            )
                            .child(
                                icon_action(
                                    SharedString::from(format!(
                                        "expand-conversation-tool-server-{}",
                                        server.id
                                    )),
                                    if expanded {
                                        AppIcon::ChevronUp
                                    } else {
                                        AppIcon::ChevronDown
                                    },
                                    IconTone::Muted,
                                    if expanded {
                                        "Collapse tool server"
                                    } else {
                                        "Expand tool server"
                                    },
                                    cx,
                                )
                                .size(px(24.0))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.toggle_conversation_tool_server(expand_id.clone(), cx)
                                    },
                                )),
                            ),
                    ),
            );

        if expanded {
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(server.tools.iter().map(|tool| {
                        let checked = server_available
                            && conversation.tool_selection.resolves(
                                &server.id,
                                &tool.name,
                                tool.enabled,
                            );
                        let server_id = server.id.clone();
                        let tool_name = tool.name.clone();
                        let label = tool.title.as_deref().unwrap_or(&tool.name).to_string();
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
                                            .child(label),
                                    )
                                    .child(
                                        Switch::new(SharedString::from(format!(
                                            "conversation-tool-{}-{}",
                                            server.id, tool.name
                                        )))
                                        .small()
                                        .checked(checked)
                                        .color(cx.theme().primary)
                                        .disabled(
                                            generating || !server_available || app.mcp.loading,
                                        )
                                        .tooltip(if server_available {
                                            "Override this tool for the conversation"
                                        } else {
                                            "The MCP server must be enabled and connected"
                                        })
                                        .on_click(
                                            cx.listener(move |this, enabled: &bool, _, cx| {
                                                this.set_conversation_tool_enabled(
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
        content = content.child(card);
    }

    if !has_tools {
        content = content.child(notice(
            "No MCP tools have been discovered. Configure them in Settings.",
            cx,
        ));
    }
    content.into_any_element()
}
