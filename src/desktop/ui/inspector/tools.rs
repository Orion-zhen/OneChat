use super::*;

mod server;

use server::render_tool_server;

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
        content = content.child(render_tool_server(
            app,
            conversation,
            server,
            generating,
            cx,
        ));
    }
    if !has_tools {
        content = content.child(notice(
            "No MCP tools have been discovered. Configure them in Settings.",
            cx,
        ));
    }
    content.into_any_element()
}
