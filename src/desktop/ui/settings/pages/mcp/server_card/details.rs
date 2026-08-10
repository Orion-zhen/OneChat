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
                                    .on_click(cx.listener(move |this, enabled: &bool, _, cx| {
                                        this.set_mcp_tool_enabled(
                                            server_id.clone(),
                                            tool_name.clone(),
                                            *enabled,
                                            cx,
                                        )
                                    })),
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

    card
}
