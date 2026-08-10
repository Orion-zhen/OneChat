use super::*;

pub(super) async fn start_stdio_server(
    config: &McpStdioServerConfig,
    command_path: PathBuf,
    environment: &ExecutionEnvironment,
) -> Result<(ServerSession, Vec<McpToolSnapshot>, Option<String>)> {
    let mut command = tokio::process::Command::new(command_path);
    command.args(&config.args);
    environment.apply(&mut command, &config.env);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Some(cwd) = &config.cwd {
        command.current_dir(cwd);
    }
    let transport = TokioChildProcess::new(command).map_err(McpError::from_display)?;
    let service = tokio::time::timeout(INITIALIZE_TIMEOUT, client_info().serve(transport))
        .await
        .map_err(|_| McpError::new("MCP server initialization timed out"))?
        .map_err(McpError::from_display)?;
    finish_server(service, &config.disabled_tools).await
}

pub(super) async fn start_http_server(
    config: &McpHttpServerConfig,
    oauth_store_path: PathBuf,
) -> Result<(ServerSession, Vec<McpToolSnapshot>, Option<String>)> {
    let client = http_client(config)?;
    let transport_config = http_transport_config(config)?;
    let service = match &config.oauth {
        Some(oauth) if oauth.flow == McpOAuthFlow::ClientCredentials => {
            let mut state = OAuthState::new(&config.url, Some(client.clone()))
                .await
                .map_err(McpError::from_display)?;
            state
                .authenticate_client_credentials(ClientCredentialsConfig::ClientSecret {
                    client_id: oauth.client_id.clone().expect("validated OAuth client ID"),
                    client_secret: oauth
                        .client_secret
                        .clone()
                        .expect("validated OAuth client secret"),
                    scopes: oauth.scopes.clone(),
                    resource: Some(config.url.clone()),
                })
                .await
                .map_err(McpError::from_display)?;
            let manager = state
                .into_authorization_manager()
                .expect("client credentials produces an authorized manager");
            let transport = StreamableHttpClientTransport::with_client(
                AuthClient::new(client, manager),
                transport_config,
            );
            tokio::time::timeout(INITIALIZE_TIMEOUT, client_info().serve(transport))
                .await
                .map_err(|_| McpError::new("MCP server initialization timed out"))?
                .map_err(McpError::from_display)?
        }
        Some(oauth) => {
            let mut manager = oauth_manager(
                config,
                oauth,
                client.clone(),
                FileCredentialStore::new(oauth_store_path, oauth_fingerprint(config)),
            )
            .await?;
            if !manager
                .initialize_from_store()
                .await
                .map_err(McpError::from_display)?
            {
                return Err(McpError::new(AUTHORIZATION_REQUIRED));
            }
            let transport = StreamableHttpClientTransport::with_client(
                AuthClient::new(client, manager),
                transport_config,
            );
            tokio::time::timeout(INITIALIZE_TIMEOUT, client_info().serve(transport))
                .await
                .map_err(|_| McpError::new("MCP server initialization timed out"))?
                .map_err(McpError::from_display)?
        }
        None => {
            let transport = StreamableHttpClientTransport::with_client(client, transport_config);
            tokio::time::timeout(INITIALIZE_TIMEOUT, client_info().serve(transport))
                .await
                .map_err(|_| McpError::new("MCP server initialization timed out"))?
                .map_err(McpError::from_display)?
        }
    };
    finish_server(service, &config.disabled_tools).await
}

pub(super) fn http_client(config: &McpHttpServerConfig) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .pool_max_idle_per_host(0)
        .redirect(reqwest::redirect::Policy::none());
    if let Some(proxy) = &config.proxy {
        builder = builder.proxy(reqwest::Proxy::all(proxy).map_err(McpError::from_display)?);
    }
    builder.build().map_err(McpError::from_display)
}

pub(super) fn http_transport_config(
    config: &McpHttpServerConfig,
) -> Result<StreamableHttpClientTransportConfig> {
    let headers = config
        .headers
        .iter()
        .map(|(name, value)| {
            let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(McpError::from_display)?;
            let value =
                reqwest::header::HeaderValue::from_str(value).map_err(McpError::from_display)?;
            Ok((name, value))
        })
        .collect::<Result<HashMap<_, _>>>()?;
    let mut transport =
        StreamableHttpClientTransportConfig::with_uri(config.url.clone()).custom_headers(headers);
    if let Some(token) = &config.bearer_token {
        transport = transport.auth_header(token.clone());
    }
    Ok(transport)
}

pub(super) async fn oauth_manager(
    config: &McpHttpServerConfig,
    _oauth: &McpOAuthConfig,
    client: reqwest::Client,
    store: FileCredentialStore,
) -> Result<AuthorizationManager> {
    let mut manager = AuthorizationManager::new(&config.url)
        .await
        .map_err(McpError::from_display)?;
    manager
        .with_client(client)
        .map_err(McpError::from_display)?;
    manager.set_credential_store(store);
    Ok(manager)
}

pub(super) async fn finish_server(
    mut service: RunningService<RoleClient, ClientInfo>,
    disabled_tools: &BTreeSet<String>,
) -> Result<(ServerSession, Vec<McpToolSnapshot>, Option<String>)> {
    let tools = match tokio::time::timeout(LIST_TOOLS_TIMEOUT, service.list_all_tools()).await {
        Ok(Ok(tools)) => tools,
        Ok(Err(error)) => {
            let _ = service.close_with_timeout(SHUTDOWN_TIMEOUT).await;
            return Err(McpError::from_display(error));
        }
        Err(_) => {
            let _ = service.close_with_timeout(SHUTDOWN_TIMEOUT).await;
            return Err(McpError::new("MCP tools/list timed out"));
        }
    };
    let implementation = service.peer_info().and_then(|info| {
        info.server_info
            .as_ref()
            .map(|implementation| format!("{} {}", implementation.name, implementation.version))
    });
    let mut tools = tools
        .into_iter()
        .map(|tool| {
            let name = tool.name.into_owned();
            McpToolSnapshot {
                enabled: !disabled_tools.contains(&name),
                name,
                title: tool.title,
                description: tool.description.map(|description| description.into_owned()),
                input_schema: Value::Object((*tool.input_schema).clone()),
            }
        })
        .collect::<Vec<_>>();
    tools.sort_by(|left, right| left.name.cmp(&right.name));
    let peer = service.peer().clone();
    Ok((ServerSession { service, peer }, tools, implementation))
}
