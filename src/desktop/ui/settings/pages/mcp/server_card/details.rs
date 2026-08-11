use super::*;

pub(super) fn append_server_details(
    mut card: gpui::Div,
    app: &OneChat,
    server: &McpServerSnapshot,
    error: Option<&str>,
    cx: &mut Context<OneChat>,
) -> gpui::Div {
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
                    let label = tool.title.as_deref().unwrap_or(&tool.name).to_string();
                    let toggle = Switch::new(SharedString::from(format!(
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
                    .on_click(cx.listener(
                        move |this, enabled: &bool, _, cx| {
                            this.set_mcp_tool_enabled(
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

    card
}
