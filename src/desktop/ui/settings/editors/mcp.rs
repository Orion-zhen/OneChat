use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum McpServerEditorMode {
    #[default]
    Configure,
    Import,
}

impl McpServerEditorMode {
    pub fn index(self) -> usize {
        match self {
            Self::Configure => 0,
            Self::Import => 1,
        }
    }

    pub fn from_index(index: usize) -> Self {
        if index == 1 {
            Self::Import
        } else {
            Self::Configure
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum McpServerTransportEditor {
    #[default]
    Stdio,
    Http,
}

impl McpServerTransportEditor {
    pub fn index(self) -> usize {
        match self {
            Self::Stdio => 0,
            Self::Http => 1,
        }
    }

    pub fn from_index(index: usize) -> Self {
        if index == 1 { Self::Http } else { Self::Stdio }
    }
}

pub struct McpEnvironmentVariableEditor {
    pub name: Entity<InputState>,
    pub value: Entity<InputState>,
}

pub struct McpServerEditor {
    original_id: Option<String>,
    enabled: bool,
    disabled_tools: BTreeSet<String>,
    pub mode: McpServerEditorMode,
    pub transport: McpServerTransportEditor,
    pub id: Entity<InputState>,
    pub command: Entity<InputState>,
    pub url: Entity<InputState>,
    pub args: Vec<Entity<InputState>>,
    pub env: Vec<McpEnvironmentVariableEditor>,
    pub cwd: Entity<InputState>,
    pub headers: Vec<McpEnvironmentVariableEditor>,
    pub proxy: Entity<InputState>,
    pub bearer_token: Entity<InputState>,
    pub oauth_flow: Option<McpOAuthFlow>,
    pub oauth_client_id: Entity<InputState>,
    pub oauth_client_secret: Entity<InputState>,
    pub oauth_scopes: Entity<InputState>,
    pub oauth_callback_port: Entity<InputState>,
}

impl McpServerEditor {
    pub fn new(
        id: Option<String>,
        server: McpServerConfig,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        let (
            transport,
            enabled,
            disabled_tools,
            command,
            url,
            arguments,
            variables,
            cwd,
            headers,
            proxy,
            bearer_token,
            oauth,
        ) = match server {
            McpServerConfig::Stdio(server) => (
                McpServerTransportEditor::Stdio,
                server.enabled,
                server.disabled_tools,
                server.command,
                String::new(),
                server.args,
                server.env,
                server
                    .cwd
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
                BTreeMap::new(),
                String::new(),
                String::new(),
                None,
            ),
            McpServerConfig::Http(server) => (
                McpServerTransportEditor::Http,
                server.enabled,
                server.disabled_tools,
                String::new(),
                server.url,
                Vec::new(),
                BTreeMap::new(),
                String::new(),
                server.headers,
                server.proxy.unwrap_or_default(),
                server.bearer_token.unwrap_or_default(),
                server.oauth,
            ),
        };
        let mut args = arguments
            .into_iter()
            .map(|argument| single_line_input(argument, "Argument", window, cx))
            .collect::<Vec<_>>();
        args.push(single_line_input("", "Argument", window, cx));
        let mut env = variables
            .into_iter()
            .map(|(name, value)| McpEnvironmentVariableEditor::new(name, value, window, cx))
            .collect::<Vec<_>>();
        env.push(McpEnvironmentVariableEditor::new("", "", window, cx));
        let mut headers = headers
            .into_iter()
            .map(|(name, value)| McpEnvironmentVariableEditor::new(name, value, window, cx))
            .collect::<Vec<_>>();
        headers.push(McpEnvironmentVariableEditor::new("", "", window, cx));
        let oauth_flow = oauth.as_ref().map(|oauth| oauth.flow);
        let oauth_client_id = oauth
            .as_ref()
            .and_then(|oauth| oauth.client_id.clone())
            .unwrap_or_default();
        let oauth_client_secret = oauth
            .as_ref()
            .and_then(|oauth| oauth.client_secret.clone())
            .unwrap_or_default();
        let oauth_scopes = oauth
            .as_ref()
            .map(|oauth| oauth.scopes.join(", "))
            .unwrap_or_default();
        let oauth_callback_port = oauth
            .and_then(|oauth| oauth.callback_port)
            .map(|port| port.to_string())
            .unwrap_or_default();
        Self {
            original_id: id.clone(),
            enabled,
            disabled_tools,
            mode: McpServerEditorMode::Configure,
            transport,
            id: single_line_input(id.unwrap_or_default(), "Server ID", window, cx),
            command: single_line_input(command, "Command", window, cx),
            url: single_line_input(url, "MCP endpoint URL", window, cx),
            args,
            env,
            cwd: single_line_input(cwd, "Optional absolute working directory", window, cx),
            headers,
            proxy: single_line_input(proxy, "Optional HTTP or SOCKS proxy URL", window, cx),
            bearer_token: masked_input(bearer_token, "Optional bearer token", window, cx),
            oauth_flow,
            oauth_client_id: single_line_input(oauth_client_id, "OAuth client ID", window, cx),
            oauth_client_secret: masked_input(
                oauth_client_secret,
                "OAuth client secret",
                window,
                cx,
            ),
            oauth_scopes: single_line_input(
                oauth_scopes,
                "Comma-separated OAuth scopes",
                window,
                cx,
            ),
            oauth_callback_port: single_line_input(
                oauth_callback_port,
                "Optional callback port (0 for automatic)",
                window,
                cx,
            ),
        }
    }

    pub fn is_new(&self) -> bool {
        self.original_id.is_none()
    }

    pub fn build(&self, cx: &App) -> Result<(String, McpServerConfig), String> {
        let id = self.id.read(cx).value().trim().to_string();
        if let Some(original_id) = &self.original_id
            && original_id != &id
        {
            return Err("An existing MCP server ID cannot be changed.".into());
        }
        let mut server = match self.transport {
            McpServerTransportEditor::Stdio => {
                let mut env = BTreeMap::new();
                for variable in self.env.iter().take(self.env.len() - 1) {
                    let name = variable.name.read(cx).value().trim().to_string();
                    let value = variable.value.read(cx).value().to_string();
                    if name.is_empty() && value.is_empty() {
                        continue;
                    }
                    if name.is_empty() {
                        return Err("Environment variable name is required.".into());
                    }
                    if env.insert(name.clone(), value).is_some() {
                        return Err(format!("Environment variable {name} is duplicated."));
                    }
                }
                let cwd = nonempty(self.cwd.read(cx).value().as_ref()).map(PathBuf::from);
                McpServerConfig::Stdio(McpStdioServerConfig {
                    enabled: self.enabled,
                    command: self.command.read(cx).value().to_string(),
                    args: self
                        .args
                        .iter()
                        .take(self.args.len() - 1)
                        .map(|argument| argument.read(cx).value().trim().to_string())
                        .filter(|argument| !argument.is_empty())
                        .collect(),
                    env,
                    cwd,
                    disabled_tools: self.disabled_tools.clone(),
                })
            }
            McpServerTransportEditor::Http => {
                let mut headers = BTreeMap::new();
                for header in self.headers.iter().take(self.headers.len() - 1) {
                    let name = header.name.read(cx).value().trim().to_string();
                    let value = header.value.read(cx).value().to_string();
                    if name.is_empty() && value.is_empty() {
                        continue;
                    }
                    if name.is_empty() {
                        return Err("HTTP header name is required.".into());
                    }
                    if headers.insert(name.clone(), value).is_some() {
                        return Err(format!("HTTP header {name} is duplicated."));
                    }
                }
                let oauth = self
                    .oauth_flow
                    .map(|flow| -> Result<McpOAuthConfig, String> {
                        let callback_port =
                            nonempty(self.oauth_callback_port.read(cx).value().as_ref())
                                .map(|port| {
                                    port.parse::<u16>().map_err(|_| {
                                        "OAuth callback port must be between 0 and 65535."
                                            .to_string()
                                    })
                                })
                                .transpose()?;
                        Ok(McpOAuthConfig {
                            flow,
                            client_id: nonempty(self.oauth_client_id.read(cx).value().as_ref()),
                            client_secret: nonempty(
                                self.oauth_client_secret.read(cx).value().as_ref(),
                            ),
                            scopes: self
                                .oauth_scopes
                                .read(cx)
                                .value()
                                .split(',')
                                .map(str::trim)
                                .filter(|scope| !scope.is_empty())
                                .map(str::to_string)
                                .collect(),
                            callback_port,
                        })
                    })
                    .transpose()?;
                McpServerConfig::Http(McpHttpServerConfig {
                    enabled: self.enabled,
                    url: self.url.read(cx).value().to_string(),
                    headers,
                    proxy: nonempty(self.proxy.read(cx).value().as_ref()),
                    bearer_token: nonempty(self.bearer_token.read(cx).value().as_ref()),
                    oauth,
                    disabled_tools: self.disabled_tools.clone(),
                })
            }
        };
        McpConfig::validate_server(&id, &mut server).map_err(|error| error.to_string())?;
        Ok((id, server))
    }

    pub fn add_argument(&mut self, window: &mut Window, cx: &mut Context<OneChat>) {
        if self
            .args
            .last()
            .is_some_and(|argument| !argument.read(cx).value().trim().is_empty())
        {
            self.args
                .push(single_line_input("", "Argument", window, cx));
        }
    }

    pub fn remove_argument(&mut self, index: usize) {
        if index + 1 < self.args.len() {
            self.args.remove(index);
        }
    }

    pub fn add_environment_variable(&mut self, window: &mut Window, cx: &mut Context<OneChat>) {
        if self
            .env
            .last()
            .is_some_and(|variable| !variable.name.read(cx).value().trim().is_empty())
        {
            self.env
                .push(McpEnvironmentVariableEditor::new("", "", window, cx));
        }
    }

    pub fn remove_environment_variable(&mut self, index: usize) {
        if index + 1 < self.env.len() {
            self.env.remove(index);
        }
    }

    pub fn add_header(&mut self, window: &mut Window, cx: &mut Context<OneChat>) {
        if self
            .headers
            .last()
            .is_some_and(|header| !header.name.read(cx).value().trim().is_empty())
        {
            self.headers
                .push(McpEnvironmentVariableEditor::new("", "", window, cx));
        }
    }

    pub fn remove_header(&mut self, index: usize) {
        if index + 1 < self.headers.len() {
            self.headers.remove(index);
        }
    }

    pub fn oauth_mode_index(&self) -> usize {
        match self.oauth_flow {
            None => 0,
            Some(McpOAuthFlow::AuthorizationCode) => 1,
            Some(McpOAuthFlow::ClientCredentials) => 2,
        }
    }

    pub fn select_oauth_mode(&mut self, index: usize) {
        self.oauth_flow = match index {
            1 => Some(McpOAuthFlow::AuthorizationCode),
            2 => Some(McpOAuthFlow::ClientCredentials),
            _ => None,
        };
    }
}

impl McpEnvironmentVariableEditor {
    fn new(
        name: impl Into<String>,
        value: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        Self {
            name: single_line_input(name, "Name", window, cx),
            value: single_line_input(value, "Value", window, cx),
        }
    }
}
