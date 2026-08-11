use super::*;

pub(super) fn render_tool_server(
    app: &OneChat,
    conversation: &Conversation,
    server: &crate::mcp::McpServerSnapshot,
    generating: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
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
                        .child(status_pill(
                            tool_label,
                            false,
                            StatusPillBackground::Background,
                            cx,
                        ))
                        .child(status_pill(
                            status,
                            all_enabled,
                            StatusPillBackground::Background,
                            cx,
                        ))
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
                            Regular
                                .icon_action(
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
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.toggle_conversation_tool_server(expand_id.clone(), cx)
                                })),
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
                    let toggle = Switch::new(SharedString::from(format!(
                        "conversation-tool-{}-{}",
                        server.id, tool.name
                    )))
                    .small()
                    .checked(checked)
                    .color(cx.theme().primary)
                    .disabled(generating || !server_available || app.mcp.loading)
                    .tooltip(if server_available {
                        "Override this tool for the conversation"
                    } else {
                        "The MCP server must be enabled and connected"
                    })
                    .on_click(cx.listener(
                        move |this, enabled: &bool, _, cx| {
                            this.set_conversation_tool_enabled(
                                server_id.clone(),
                                tool_name.clone(),
                                *enabled,
                                cx,
                            )
                        },
                    ));
                    mcp_tool_row(label, tool.description.clone().map(Into::into), toggle, cx)
                })),
        );
    }
    card.into_any_element()
}
