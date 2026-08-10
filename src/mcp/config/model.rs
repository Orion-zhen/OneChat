use super::*;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct McpConfig {
    #[serde(rename = "mcpServers")]
    pub servers: BTreeMap<String, McpServerConfig>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum McpServerConfig {
    Http(McpHttpServerConfig),
    Stdio(McpStdioServerConfig),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct McpHttpServerConfig {
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    pub url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub proxy: Option<String>,
    #[serde(default, rename = "bearerToken")]
    pub bearer_token: Option<String>,
    #[serde(default)]
    pub oauth: Option<McpOAuthConfig>,
    #[serde(default, rename = "disabledTools")]
    pub disabled_tools: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum McpOAuthFlow {
    #[default]
    AuthorizationCode,
    ClientCredentials,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct McpOAuthConfig {
    pub flow: McpOAuthFlow,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_port: Option<u16>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct McpStdioServerConfig {
    pub enabled: bool,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<PathBuf>,
    #[serde(rename = "disabledTools")]
    pub disabled_tools: BTreeSet<String>,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self::Stdio(McpStdioServerConfig::default())
    }
}

impl Default for McpStdioServerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            command: String::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
            disabled_tools: BTreeSet::new(),
        }
    }
}

impl McpServerConfig {
    pub fn enabled(&self) -> bool {
        match self {
            Self::Http(server) => server.enabled,
            Self::Stdio(server) => server.enabled,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        match self {
            Self::Http(server) => server.enabled = enabled,
            Self::Stdio(server) => server.enabled = enabled,
        }
    }

    pub fn disabled_tools(&self) -> &BTreeSet<String> {
        match self {
            Self::Http(server) => &server.disabled_tools,
            Self::Stdio(server) => &server.disabled_tools,
        }
    }

    pub(super) fn disabled_tools_mut(&mut self) -> &mut BTreeSet<String> {
        match self {
            Self::Http(server) => &mut server.disabled_tools,
            Self::Stdio(server) => &mut server.disabled_tools,
        }
    }
}
