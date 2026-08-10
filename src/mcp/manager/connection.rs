use super::*;

pub(super) async fn connect_server(
    id: String,
    config: McpServerConfig,
    oauth_cache_dir: PathBuf,
    environment: ExecutionEnvironment,
) -> Connection {
    let transport = match &config {
        McpServerConfig::Http(server) => McpServerTransportSnapshot::Http {
            url: server.url.clone(),
        },
        McpServerConfig::Stdio(server) => McpServerTransportSnapshot::Stdio {
            command: server.command.clone(),
            resolved_command: environment.resolve(
                &server.command,
                server.cwd.as_deref(),
                &server.env,
            ),
        },
    };
    let interactive_oauth = matches!(
        &config,
        McpServerConfig::Http(McpHttpServerConfig {
            oauth: Some(McpOAuthConfig {
                flow: McpOAuthFlow::AuthorizationCode,
                ..
            }),
            ..
        })
    );
    let mut snapshot = McpServerSnapshot {
        id,
        enabled: config.enabled(),
        interactive_oauth,
        transport,
        status: McpServerStatus::Disabled,
        implementation: None,
        tools: Vec::new(),
    };
    if !config.enabled() {
        return Connection {
            snapshot,
            session: None,
        };
    }

    let connection = match &config {
        McpServerConfig::Http(server) => {
            start_http_server(server, oauth_store_path(&oauth_cache_dir, &snapshot.id)).await
        }
        McpServerConfig::Stdio(server) => {
            let McpServerTransportSnapshot::Stdio {
                resolved_command: Some(command_path),
                ..
            } = &snapshot.transport
            else {
                let command = Path::new(&server.command);
                let error = if !command.is_absolute()
                    && command.components().count() > 1
                    && server.cwd.is_none()
                {
                    format!(
                        "Relative executable path requires an MCP server cwd: {}",
                        server.command
                    )
                } else if command.components().count() > 1 {
                    format!(
                        "Executable is missing or not executable: {}",
                        server.command
                    )
                } else {
                    format!(
                        "Executable not found in the system execution PATH: {}",
                        server.command
                    )
                };
                snapshot.status = McpServerStatus::Failed(error);
                return Connection {
                    snapshot,
                    session: None,
                };
            };
            start_stdio_server(server, command_path.clone(), &environment).await
        }
    };

    match connection {
        Ok((session, tools, implementation)) => {
            snapshot.status = McpServerStatus::Ready;
            snapshot.tools = tools;
            snapshot.implementation = implementation;
            Connection {
                snapshot,
                session: Some(session),
            }
        }
        Err(error) => {
            snapshot.status = if error.to_string() == AUTHORIZATION_REQUIRED {
                McpServerStatus::AuthorizationRequired
            } else {
                McpServerStatus::Failed(error.to_string())
            };
            Connection {
                snapshot,
                session: None,
            }
        }
    }
}

pub(super) fn client_info() -> ClientInfo {
    let mut client_info = ClientInfo::default();
    client_info.client_info = Implementation::new("OneChat", env!("CARGO_PKG_VERSION"));
    client_info
}
