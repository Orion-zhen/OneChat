use super::*;

pub(in crate::desktop::ui::settings) fn mcp_server_form(
    editor: &McpServerEditor,
    json_import: &Entity<InputState>,
    loading: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let title = if editor.is_new() {
        "Add MCP Server"
    } else {
        "Edit MCP Server"
    };
    let arguments =
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .children(editor.args.iter().enumerate().map(|(index, argument)| {
                let is_draft = index + 1 == editor.args.len();
                let action = if is_draft {
                    icon_action(
                        "add-mcp-argument",
                        AppIcon::Plus,
                        IconTone::Accent,
                        "Add argument",
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.add_mcp_argument(window, cx)))
                } else {
                    icon_action(
                        SharedString::from(format!("remove-mcp-argument-{index}")),
                        AppIcon::Trash,
                        IconTone::Danger,
                        "Remove argument",
                        cx,
                    )
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.remove_mcp_argument(index, cx)),
                    )
                };
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(form_input(argument, "MCP server argument")),
                    )
                    .child(action)
            }));
    let environment =
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .children(editor.env.iter().enumerate().map(|(index, variable)| {
                let is_draft = index + 1 == editor.env.len();
                let action = if is_draft {
                    icon_action(
                        "add-mcp-environment",
                        AppIcon::Plus,
                        IconTone::Accent,
                        "Add environment variable",
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.add_mcp_environment_variable(window, cx)
                    }))
                } else {
                    icon_action(
                        SharedString::from(format!("remove-mcp-environment-{index}")),
                        AppIcon::Trash,
                        IconTone::Danger,
                        "Remove environment variable",
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.remove_mcp_environment_variable(index, cx)
                    }))
                };
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(form_input(&variable.name, "Environment variable name")),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(form_input(&variable.value, "Environment variable value")),
                    )
                    .child(action)
            }));
    let headers =
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .children(editor.headers.iter().enumerate().map(|(index, header)| {
                let is_draft = index + 1 == editor.headers.len();
                let action = if is_draft {
                    icon_action(
                        "add-mcp-header",
                        AppIcon::Plus,
                        IconTone::Accent,
                        "Add HTTP header",
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.add_mcp_header(window, cx)))
                } else {
                    icon_action(
                        SharedString::from(format!("remove-mcp-header-{index}")),
                        AppIcon::Trash,
                        IconTone::Danger,
                        "Remove HTTP header",
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.remove_mcp_header(index, cx)))
                };
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(form_input(&header.name, "HTTP header name")),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(form_input(&header.value, "HTTP header value")),
                    )
                    .child(action)
            }));
    let oauth_selector = div()
        .w_full()
        .flex()
        .items_center()
        .gap_1()
        .rounded(px(10.0))
        .bg(cx.theme().muted)
        .p_1()
        .children(
            [
                ("mcp-oauth-none", "None"),
                ("mcp-oauth-code", "Browser login"),
                ("mcp-oauth-credentials", "Client credentials"),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (id, label))| {
                let selected = editor.oauth_mode_index() == index;
                Button::new(id)
                    .ghost()
                    .flex_1()
                    .h(px(32.0))
                    .rounded(px(7.0))
                    .label(label)
                    .selected(selected)
                    .toggled(selected)
                    .when(selected, |button| button.bg(cx.theme().popover))
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.select_mcp_oauth_mode(index, cx)),
                    )
            }),
        );
    let transport_selector = div()
        .w_full()
        .flex()
        .items_center()
        .gap_1()
        .rounded(px(10.0))
        .bg(cx.theme().muted)
        .p_1()
        .children(
            [
                ("mcp-transport-stdio", "Local command"),
                ("mcp-transport-http", "HTTP URL"),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (id, label))| {
                let selected = editor.transport.index() == index;
                Button::new(id)
                    .ghost()
                    .flex_1()
                    .h(px(32.0))
                    .rounded(px(7.0))
                    .label(label)
                    .selected(selected)
                    .toggled(selected)
                    .when(selected, |button| button.bg(cx.theme().popover))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_mcp_server_transport(index, cx)
                    }))
            }),
        );
    let fields = Form::vertical()
        .columns(2)
        .child(
            Field::new()
                .label("Server ID")
                .required(true)
                .description(if editor.is_new() {
                    "Unique name used to identify this server"
                } else {
                    "Existing server IDs cannot be renamed"
                })
                .child(form_input(&editor.id, "MCP server ID")),
        )
        .child(
            Field::new()
                .label("Transport")
                .description("Run a local stdio process or connect over Streamable HTTP")
                .child(transport_selector),
        );
    let fields = match editor.transport {
        McpServerTransportEditor::Stdio => fields
            .child(
                Field::new()
                    .label("Command")
                    .required(true)
                    .description("Executable resolved from the system execution PATH")
                    .child(form_input(&editor.command, "MCP server command")),
            )
            .child(
                Field::new()
                    .label("Arguments")
                    .description("Command-line arguments in execution order")
                    .child(arguments),
            )
            .child(
                Field::new()
                    .label("Environment")
                    .description("Variable name and value pairs")
                    .child(environment),
            )
            .child(
                Field::new()
                    .label("Working Directory")
                    .description("Optional absolute path")
                    .child(form_input(&editor.cwd, "MCP server working directory")),
            ),
        McpServerTransportEditor::Http => {
            let fields = fields
                .child(
                    Field::new()
                        .label("URL")
                        .required(true)
                        .description("Streamable HTTP MCP endpoint")
                        .col_span(2)
                        .child(form_input(&editor.url, "MCP server URL")),
                )
                .child(
                    Field::new()
                        .label("Headers")
                        .description("Custom headers sent with every MCP request")
                        .col_span(2)
                        .child(headers),
                )
                .child(
                    Field::new()
                        .label("Proxy")
                        .description("Optional HTTP or SOCKS proxy")
                        .child(form_input(&editor.proxy, "MCP HTTP proxy")),
                )
                .child(
                    Field::new()
                        .label("Bearer Token")
                        .description("Cannot be combined with OAuth or Authorization header")
                        .child(form_input(&editor.bearer_token, "MCP bearer token")),
                )
                .child(
                    Field::new()
                        .label("OAuth")
                        .description(
                            "Interactive authorization code or machine-to-machine credentials",
                        )
                        .col_span(2)
                        .child(oauth_selector),
                );
            match editor.oauth_flow {
                None => fields,
                Some(flow) => {
                    let fields = fields
                        .child(
                            Field::new()
                                .label("OAuth Client ID")
                                .required(flow == McpOAuthFlow::ClientCredentials)
                                .description(if flow == McpOAuthFlow::AuthorizationCode {
                                    "Optional; omitted to use dynamic client registration"
                                } else {
                                    "Required for client credentials"
                                })
                                .child(form_input(&editor.oauth_client_id, "OAuth client ID")),
                        )
                        .child(
                            Field::new()
                                .label("OAuth Client Secret")
                                .required(flow == McpOAuthFlow::ClientCredentials)
                                .description("Optional for public browser clients")
                                .child(form_input(
                                    &editor.oauth_client_secret,
                                    "OAuth client secret",
                                )),
                        )
                        .child(
                            Field::new()
                                .label("OAuth Scopes")
                                .description("Comma-separated; empty uses server defaults")
                                .child(form_input(&editor.oauth_scopes, "OAuth scopes")),
                        );
                    if flow == McpOAuthFlow::AuthorizationCode {
                        fields.child(
                            Field::new()
                                .label("Callback Port")
                                .description("Optional; 0 or empty selects an available port")
                                .child(form_input(
                                    &editor.oauth_callback_port,
                                    "OAuth callback port",
                                )),
                        )
                    } else {
                        fields
                    }
                }
            }
        }
    };
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
                        Input::new(json_import)
                            .large()
                            .aria_label("MCP JSON configuration")
                            .h(px(220.0)),
                    ),
            )
            .into_any_element()
    } else {
        fields.into_any_element()
    };
    let save = if editor.is_new() && editor.mode == McpServerEditorMode::Import {
        primary_icon_action(
            "import-mcp-server",
            AppIcon::Save,
            "Import MCP configuration",
            cx,
        )
        .disabled(loading)
        .on_click(cx.listener(|this, _, _, cx| this.import_mcp_servers(cx)))
    } else {
        primary_icon_action("save-mcp-server", AppIcon::Save, "Save MCP server", cx)
            .disabled(loading)
            .on_click(cx.listener(|this, _, _, cx| this.save_mcp_server(cx)))
    };

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            icon_action(
                                "cancel-mcp-server",
                                AppIcon::Close,
                                IconTone::Muted,
                                "Cancel",
                                cx,
                            )
                            .on_click(
                                cx.listener(|this, _, _, cx| this.cancel_mcp_server_editor(cx)),
                            ),
                        )
                        .child(save),
                ),
        )
        .children(mode_selector)
        .child(body)
        .into_any_element()
}
