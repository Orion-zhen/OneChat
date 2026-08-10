use super::super::*;

mod server_card;

use server_card::{mcp_executable_row, mcp_server_card};

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
