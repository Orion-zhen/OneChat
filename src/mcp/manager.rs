use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    env, fs,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use futures_util::future::join_all;
use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, CallToolResponse, CallToolResult, ClientInfo, Implementation},
    service::{Peer, RunningService},
    transport::{
        AuthClient, AuthorizationManager, AuthorizationRequest, ClientCredentialsConfig,
        CredentialStore, StoredCredentials, StreamableHttpClientTransport, TokioChildProcess,
        auth::OAuthState, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Map, Value};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use super::{
    McpConfig, McpError, McpHttpServerConfig, McpOAuthConfig, McpOAuthFlow, McpServerConfig,
    McpStdioServerConfig, Result, executable::ExecutionEnvironment,
};

const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(30);
const LIST_TOOLS_TIMEOUT: Duration = Duration::from_secs(30);
const CALL_TOOL_TIMEOUT: Duration = Duration::from_secs(120);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const OAUTH_CALLBACK_TIMEOUT: Duration = Duration::from_secs(120);
const AUTHORIZATION_REQUIRED: &str = "OAuth authorization required";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum McpServerStatus {
    Disabled,
    AuthorizationRequired,
    Ready,
    Failed(String),
    Stopped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpExecutableSnapshot {
    pub name: String,
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct McpToolSnapshot {
    pub name: String,
    pub enabled: bool,
    pub title: Option<String>,
    pub description: Option<String>,
    pub input_schema: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct McpToolDefinition {
    pub name: String,
    pub server_id: String,
    pub enabled: bool,
    pub tool_name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum McpServerTransportSnapshot {
    Stdio {
        command: String,
        resolved_command: Option<PathBuf>,
    },
    Http {
        url: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct McpServerSnapshot {
    pub id: String,
    pub enabled: bool,
    pub interactive_oauth: bool,
    pub transport: McpServerTransportSnapshot,
    pub status: McpServerStatus,
    pub implementation: Option<String>,
    pub tools: Vec<McpToolSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct McpSnapshot {
    pub config_path: PathBuf,
    pub config_error: Option<String>,
    pub executables: Vec<McpExecutableSnapshot>,
    pub servers: Vec<McpServerSnapshot>,
}

impl McpSnapshot {
    pub fn empty(config_path: impl Into<PathBuf>) -> Self {
        Self {
            config_path: config_path.into(),
            config_error: None,
            executables: executable_snapshots(&ExecutionEnvironment::inherited()),
            servers: Vec::new(),
        }
    }
}

pub struct McpManager {
    config_path: PathBuf,
    state: RwLock<ManagerState>,
    reload: Mutex<()>,
}

struct ManagerState {
    snapshot: McpSnapshot,
    sessions: BTreeMap<String, ServerSession>,
    tools: BTreeMap<String, ToolRoute>,
}

struct ToolRoute {
    server_id: String,
    tool_name: String,
    definition: McpToolDefinition,
}

struct ServerSession {
    service: RunningService<RoleClient, ClientInfo>,
    peer: Peer<RoleClient>,
}

struct Connection {
    snapshot: McpServerSnapshot,
    session: Option<ServerSession>,
}

impl McpManager {
    pub fn new(config_path: impl Into<PathBuf>) -> Self {
        let config_path = config_path.into();
        Self {
            state: RwLock::new(ManagerState {
                snapshot: McpSnapshot::empty(config_path.clone()),
                sessions: BTreeMap::new(),
                tools: BTreeMap::new(),
            }),
            config_path,
            reload: Mutex::new(()),
        }
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub async fn snapshot(&self) -> McpSnapshot {
        self.state.read().await.snapshot.clone()
    }

    pub async fn reload(&self) -> McpSnapshot {
        let _reload = self.reload.lock().await;
        self.reload_inner().await
    }

    pub async fn upsert_server(&self, id: String, server: McpServerConfig) -> Result<McpSnapshot> {
        let _reload = self.reload.lock().await;
        McpConfig::upsert(&self.config_path, &id, server)?;
        Ok(self.reload_inner().await)
    }

    pub async fn import_servers(&self, source: String) -> Result<(usize, McpSnapshot)> {
        let _reload = self.reload.lock().await;
        let count = McpConfig::import(&self.config_path, &source)?;
        Ok((count, self.reload_inner().await))
    }

    pub async fn delete_server(&self, id: String) -> Result<McpSnapshot> {
        let _reload = self.reload.lock().await;
        McpConfig::delete(&self.config_path, &id)?;
        let _ = fs::remove_file(oauth_store_path(&oauth_cache_dir(&self.config_path), &id));
        Ok(self.reload_inner().await)
    }

    pub async fn set_server_enabled(&self, id: String, enabled: bool) -> Result<McpSnapshot> {
        let _reload = self.reload.lock().await;
        McpConfig::set_server_enabled(&self.config_path, &id, enabled)?;
        Ok(self.reload_inner().await)
    }

    pub async fn set_tool_enabled(
        &self,
        server_id: String,
        tool_name: String,
        enabled: bool,
    ) -> Result<McpSnapshot> {
        let _reload = self.reload.lock().await;
        let exists = self
            .state
            .read()
            .await
            .snapshot
            .servers
            .iter()
            .find(|server| server.id == server_id)
            .is_some_and(|server| server.tools.iter().any(|tool| tool.name == tool_name));
        if !exists {
            return Err(McpError::new(format!(
                "MCP tool not found: {server_id}.{tool_name}"
            )));
        }
        McpConfig::set_tool_enabled(&self.config_path, &server_id, &tool_name, enabled)?;

        let mut state = self.state.write().await;
        let tool = state
            .snapshot
            .servers
            .iter_mut()
            .find(|server| server.id == server_id)
            .and_then(|server| server.tools.iter_mut().find(|tool| tool.name == tool_name))
            .expect("tool existence checked while reload lock is held");
        tool.enabled = enabled;
        state.tools = tool_routes(&state.snapshot.servers);
        Ok(state.snapshot.clone())
    }

    pub async fn authorize_server(
        &self,
        id: String,
        authorization_url: async_channel::Sender<String>,
    ) -> Result<McpSnapshot> {
        let _reload = self.reload.lock().await;
        let config = McpConfig::load(&self.config_path)?
            .servers
            .remove(&id)
            .ok_or_else(|| McpError::new(format!("MCP server not found: {id}")))?;
        let McpServerConfig::Http(config) = config else {
            return Err(McpError::new("Only HTTP MCP servers support OAuth"));
        };
        let oauth = config
            .oauth
            .as_ref()
            .filter(|oauth| oauth.flow == McpOAuthFlow::AuthorizationCode)
            .ok_or_else(|| {
                McpError::new("MCP server is not configured for authorization code OAuth")
            })?;
        let listener = tokio::net::TcpListener::bind((
            std::net::Ipv4Addr::LOCALHOST,
            oauth.callback_port.unwrap_or_default(),
        ))
        .await
        .map_err(McpError::from_display)?;
        let redirect_uri = format!(
            "http://127.0.0.1:{}/callback",
            listener
                .local_addr()
                .map_err(McpError::from_display)?
                .port()
        );
        let client = http_client(&config)?;
        let store = FileCredentialStore::new(
            oauth_store_path(&oauth_cache_dir(&self.config_path), &id),
            oauth_fingerprint(&config),
        );
        let manager = oauth_manager(&config, oauth, client, store).await?;
        let mut state = OAuthState::Unauthorized(manager);
        let mut request = AuthorizationRequest::new(&redirect_uri)
            .with_scopes(oauth.scopes.clone())
            .with_client_name("OneChat");
        if let Some(client_id) = &oauth.client_id {
            request = request.with_preregistered_client(client_id);
        }
        if let Some(client_secret) = &oauth.client_secret {
            request = request.with_client_secret(client_secret);
        }
        state
            .start_authorization(request)
            .await
            .map_err(McpError::from_display)?;
        authorization_url
            .send(
                state
                    .get_authorization_url()
                    .await
                    .map_err(McpError::from_display)?,
            )
            .await
            .map_err(|_| McpError::new("OAuth authorization was cancelled"))?;
        let (code, csrf_token) = receive_oauth_callback(listener).await?;
        state
            .handle_callback(&code, &csrf_token)
            .await
            .map_err(McpError::from_display)?;
        Ok(self.reload_inner().await)
    }

    pub async fn test_server(&self, id: String) -> Result<()> {
        let _reload = self.reload.lock().await;
        let mut config = McpConfig::load(&self.config_path)?
            .servers
            .remove(&id)
            .ok_or_else(|| McpError::new(format!("MCP server not found: {id}")))?;
        config.set_enabled(true);
        let mut connection = connect_server(
            id,
            config,
            oauth_cache_dir(&self.config_path),
            ExecutionEnvironment::discover().await,
        )
        .await;
        let result = match &connection.snapshot.status {
            McpServerStatus::Ready => Ok(()),
            McpServerStatus::AuthorizationRequired => Err(McpError::new(AUTHORIZATION_REQUIRED)),
            McpServerStatus::Failed(error) => Err(McpError::new(error.clone())),
            status => Err(McpError::new(format!(
                "MCP server test ended with status: {status:?}"
            ))),
        };
        if let Some(mut session) = connection.session.take() {
            let _ = session.service.close_with_timeout(SHUTDOWN_TIMEOUT).await;
        }
        result
    }

    async fn reload_inner(&self) -> McpSnapshot {
        let environment = ExecutionEnvironment::discover().await;
        let config = match McpConfig::load(&self.config_path) {
            Ok(config) => config,
            Err(error) => {
                let mut state = self.state.write().await;
                state.snapshot.config_error = Some(error.to_string());
                state.snapshot.executables = executable_snapshots(&environment);
                return state.snapshot.clone();
            }
        };

        let connections = join_all(config.servers.into_iter().map(|(id, server)| {
            connect_server(
                id,
                server,
                oauth_cache_dir(&self.config_path),
                environment.clone(),
            )
        }))
        .await;
        let mut snapshot = McpSnapshot::empty(self.config_path.clone());
        snapshot.executables = executable_snapshots(&environment);
        let mut sessions = BTreeMap::new();
        for connection in connections {
            if let Some(session) = connection.session {
                sessions.insert(connection.snapshot.id.clone(), session);
            }
            snapshot.servers.push(connection.snapshot);
        }

        let tools = tool_routes(&snapshot.servers);
        let old_sessions = {
            let mut state = self.state.write().await;
            let old_sessions = std::mem::replace(&mut state.sessions, sessions);
            state.snapshot = snapshot.clone();
            state.tools = tools;
            old_sessions
        };
        close_sessions(old_sessions).await;
        snapshot
    }

    pub async fn tools(&self) -> Vec<McpToolDefinition> {
        self.state
            .read()
            .await
            .tools
            .values()
            .filter(|route| route.definition.enabled)
            .map(|route| route.definition.clone())
            .collect()
    }

    pub async fn all_tools(&self) -> Vec<McpToolDefinition> {
        self.state
            .read()
            .await
            .tools
            .values()
            .map(|route| route.definition.clone())
            .collect()
    }

    pub async fn call_model_tool(
        &self,
        name: &str,
        arguments: Map<String, Value>,
        cancellation: CancellationToken,
    ) -> Result<CallToolResult> {
        let (server_id, tool_name) = self
            .state
            .read()
            .await
            .tools
            .get(name)
            .filter(|route| route.definition.enabled)
            .map(|route| (route.server_id.clone(), route.tool_name.clone()))
            .ok_or_else(|| McpError::new(format!("Unknown MCP tool: {name}")))?;
        self.call_tool(&server_id, &tool_name, arguments, cancellation)
            .await
    }

    pub async fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        arguments: Map<String, Value>,
        cancellation: CancellationToken,
    ) -> Result<CallToolResult> {
        let peer = self
            .state
            .read()
            .await
            .sessions
            .get(server_id)
            .map(|session| session.peer.clone())
            .ok_or_else(|| McpError::new(format!("MCP server is not ready: {server_id}")))?;
        let call = peer.call_tool_once(
            CallToolRequestParams::new(tool_name.to_string()).with_arguments(arguments),
        );
        let response = tokio::select! {
            _ = cancellation.cancelled() => return Err(McpError::new("MCP tool call cancelled")),
            response = tokio::time::timeout(CALL_TOOL_TIMEOUT, call) => response,
        }
        .map_err(|_| McpError::new(format!("MCP tool call timed out: {server_id}.{tool_name}")))?
        .map_err(McpError::from_display)?;

        match response {
            CallToolResponse::Complete(result) => Ok(result),
            CallToolResponse::InputRequired(_) => Err(McpError::new(
                "MCP tool requested input, which OneChat does not support",
            )),
            _ => Err(McpError::new(
                "MCP server returned an unsupported tool response",
            )),
        }
    }

    pub async fn shutdown(&self) {
        let sessions = {
            let mut state = self.state.write().await;
            for server in &mut state.snapshot.servers {
                if server.status == McpServerStatus::Ready {
                    server.status = McpServerStatus::Stopped;
                }
            }
            state.tools.clear();
            std::mem::take(&mut state.sessions)
        };
        close_sessions(sessions).await;
    }
}

async fn connect_server(
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

fn client_info() -> ClientInfo {
    let mut client_info = ClientInfo::default();
    client_info.client_info = Implementation::new("OneChat", env!("CARGO_PKG_VERSION"));
    client_info
}

async fn start_stdio_server(
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

async fn start_http_server(
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

fn http_client(config: &McpHttpServerConfig) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .pool_max_idle_per_host(0)
        .redirect(reqwest::redirect::Policy::none());
    if let Some(proxy) = &config.proxy {
        builder = builder.proxy(reqwest::Proxy::all(proxy).map_err(McpError::from_display)?);
    }
    builder.build().map_err(McpError::from_display)
}

fn http_transport_config(
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

async fn oauth_manager(
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

async fn finish_server(
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

fn tool_routes(servers: &[McpServerSnapshot]) -> BTreeMap<String, ToolRoute> {
    servers
        .iter()
        .filter(|server| server.status == McpServerStatus::Ready)
        .flat_map(|server| {
            server.tools.iter().map(move |tool| {
                let name = model_tool_name(&server.id, &tool.name);
                let description = tool
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("MCP tool {} from server {}", tool.name, server.id));
                (
                    name.clone(),
                    ToolRoute {
                        server_id: server.id.clone(),
                        tool_name: tool.name.clone(),
                        definition: McpToolDefinition {
                            name,
                            server_id: server.id.clone(),
                            enabled: tool.enabled,
                            tool_name: tool.name.clone(),
                            description,
                            input_schema: tool.input_schema.clone(),
                        },
                    },
                )
            })
        })
        .collect()
}

fn model_tool_name(server_id: &str, tool_name: &str) -> String {
    const MAX_LEN: usize = 64;
    let raw = format!("{server_id}__{tool_name}");
    let valid = raw.len() <= MAX_LEN
        && raw
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
        && raw
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_');
    if valid {
        return raw;
    }

    let mut safe = raw
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if !safe
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
    {
        safe.insert(0, '_');
    }
    let hash = raw.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    let suffix = format!("_{:08x}", hash as u32);
    safe.truncate(MAX_LEN - suffix.len());
    safe.push_str(&suffix);
    safe
}

async fn close_sessions(sessions: BTreeMap<String, ServerSession>) {
    for (_, mut session) in sessions {
        let _ = session.service.close_with_timeout(SHUTDOWN_TIMEOUT).await;
    }
}

fn executable_snapshots(environment: &ExecutionEnvironment) -> Vec<McpExecutableSnapshot> {
    ["npx", "uv"]
        .into_iter()
        .map(|name| McpExecutableSnapshot {
            name: name.to_string(),
            path: environment.resolve(name, None, &BTreeMap::new()),
        })
        .collect()
}

async fn receive_oauth_callback(listener: tokio::net::TcpListener) -> Result<(String, String)> {
    let (mut stream, _) = tokio::time::timeout(OAUTH_CALLBACK_TIMEOUT, listener.accept())
        .await
        .map_err(|_| McpError::new("OAuth authorization timed out"))?
        .map_err(McpError::from_display)?;
    let mut request = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        use tokio::io::AsyncReadExt as _;
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(McpError::from_display)?;
        if read == 0 || request.len() + read > 64 * 1024 {
            return Err(McpError::new("Invalid OAuth callback request"));
        }
        request.extend_from_slice(&chunk[..read]);
        if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            break;
        }
    }
    let first_line = String::from_utf8_lossy(&request)
        .lines()
        .next()
        .ok_or_else(|| McpError::new("Invalid OAuth callback request"))?
        .to_string();
    let target = first_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| McpError::new("Invalid OAuth callback request"))?;
    let url = reqwest::Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(McpError::from_display)?;
    if url.path() != "/callback" {
        return Err(McpError::new("Invalid OAuth callback path"));
    }
    let mut code = None;
    let mut state = None;
    let mut oauth_error = None;
    for (name, value) in url.query_pairs() {
        match name.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => oauth_error = Some(value.into_owned()),
            _ => {}
        }
    }
    let result = match oauth_error {
        Some(error) => Err(McpError::new(format!(
            "OAuth authorization failed: {error}"
        ))),
        None => Ok((
            code.ok_or_else(|| McpError::new("OAuth callback is missing code"))?,
            state.ok_or_else(|| McpError::new("OAuth callback is missing state"))?,
        )),
    };
    let (status, message) = if result.is_ok() {
        (
            "200 OK",
            "Authorization complete. You can close this tab and return to OneChat.",
        )
    } else {
        (
            "400 Bad Request",
            "Authorization failed. Return to OneChat and try again.",
        )
    };
    let body = format!("<html><body><h2>{message}</h2></body></html>");
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    use tokio::io::AsyncWriteExt as _;
    let _ = stream.write_all(response.as_bytes()).await;
    result
}

#[derive(Clone)]
struct FileCredentialStore {
    path: PathBuf,
    fingerprint: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct StoredOAuthCredentials {
    fingerprint: String,
    credentials: StoredCredentials,
}

impl FileCredentialStore {
    fn new(path: PathBuf, fingerprint: String) -> Self {
        Self { path, fingerprint }
    }

    fn error(error: impl std::fmt::Display) -> rmcp::transport::AuthError {
        rmcp::transport::AuthError::InternalError(error.to_string())
    }
}

#[async_trait::async_trait]
impl CredentialStore for FileCredentialStore {
    async fn load(
        &self,
    ) -> std::result::Result<Option<StoredCredentials>, rmcp::transport::AuthError> {
        let source = match fs::read_to_string(&self.path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Self::error(error)),
        };
        let stored: StoredOAuthCredentials = serde_json::from_str(&source).map_err(Self::error)?;
        if stored.fingerprint != self.fingerprint {
            let _ = fs::remove_file(&self.path);
            return Ok(None);
        }
        Ok(Some(stored.credentials))
    }

    async fn save(
        &self,
        credentials: StoredCredentials,
    ) -> std::result::Result<(), rmcp::transport::AuthError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(Self::error)?;
        }
        let source = serde_json::to_vec_pretty(&StoredOAuthCredentials {
            fingerprint: self.fingerprint.clone(),
            credentials,
        })
        .map_err(Self::error)?;
        let mut options = fs::OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        use std::io::Write as _;
        let mut file = options.open(&self.path).map_err(Self::error)?;
        #[cfg(unix)]
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(Self::error)?;
        file.write_all(&source).map_err(Self::error)
    }

    async fn clear(&self) -> std::result::Result<(), rmcp::transport::AuthError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Self::error(error)),
        }
    }
}

fn oauth_cache_dir(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("mcp-oauth")
}

fn oauth_store_path(cache_dir: &Path, server_id: &str) -> PathBuf {
    cache_dir.join(format!("{:016x}.json", stable_hash(server_id)))
}

fn oauth_fingerprint(config: &McpHttpServerConfig) -> String {
    let oauth = serde_json::to_string(&config.oauth).expect("OAuth config serialization");
    format!("{:016x}", stable_hash(&format!("{}\n{oauth}", config.url)))
}

fn stable_hash(value: &str) -> u64 {
    value.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}
