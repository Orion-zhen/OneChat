use super::collections::{argument_fields, environment_fields, header_fields};
use super::*;

pub(super) fn server_fields(editor: &McpServerEditor, cx: &mut Context<OneChat>) -> AnyElement {
    let arguments = argument_fields(editor, cx);
    let environment = environment_fields(editor, cx);
    let headers = header_fields(editor, cx);
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
    fields.into_any_element()
}
