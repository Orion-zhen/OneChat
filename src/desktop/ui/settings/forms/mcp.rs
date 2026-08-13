use super::*;

mod collections;
mod transport;

use transport::server_fields;

pub(in crate::desktop::ui::settings) fn mcp_server_form(
    editor: &McpServerEditor,
    json_import: &Entity<TextareaState>,
    loading: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let title = if editor.is_new() {
        "Add MCP Server"
    } else {
        "Edit MCP Server"
    };
    let fields = server_fields(editor, cx);
    let mode_selector = editor.is_new().then(|| {
        div()
            .w_full()
            .flex()
            .items_center()
            .gap_1()
            .rounded(px(12.0))
            .bg(cx.theme().muted)
            .p_1()
            .children(
                [
                    ("mcp-mode-configure", "Configure"),
                    ("mcp-mode-import", "Import"),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, (id, label))| {
                    let selected = editor.mode.index() == index;
                    Button::new(id)
                        .ghost()
                        .large()
                        .flex_1()
                        .h(px(40.0))
                        .rounded(px(9.0))
                        .label(label)
                        .selected(selected)
                        .toggled(selected)
                        .when(selected, |button| {
                            button
                                .bg(cx.theme().popover)
                                .font_weight(FontWeight::SEMIBOLD)
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_mcp_server_editor_mode(index, cx)
                        }))
                }),
            )
    });
    let body = if editor.is_new() && editor.mode == McpServerEditorMode::Import {
        Form::vertical()
            .child(
                Field::new()
                    .label("JSON / JSONC")
                    .description("Paste an mcpServers object; matching server IDs are replaced")
                    .child(
                        Textarea::new(json_import)
                            .aria_label("MCP JSON configuration")
                            .h(px(220.0)),
                    ),
            )
            .into_any_element()
    } else {
        fields
    };
    let save = if editor.is_new() && editor.mode == McpServerEditorMode::Import {
        Compact
            .primary_icon_action(
                "import-mcp-server",
                AppIcon::Save,
                "Import MCP configuration",
                cx,
            )
            .disabled(loading)
            .on_click(cx.listener(|this, _, _, cx| this.import_mcp_servers(cx)))
    } else {
        Compact
            .primary_icon_action("save-mcp-server", AppIcon::Save, "Save MCP server", cx)
            .disabled(loading)
            .on_click(cx.listener(|this, _, _, cx| this.save_mcp_server(cx)))
    };

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_4()
        .child(editor_header(
            title,
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Compact
                        .icon_action(
                            "cancel-mcp-server",
                            AppIcon::Close,
                            IconTone::Muted,
                            "Cancel",
                            cx,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_mcp_server_editor(cx))),
                )
                .child(save),
        ))
        .children(mode_selector)
        .child(body)
        .into_any_element()
}
