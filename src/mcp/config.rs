use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use jsonc_parser::{
    ParseOptions,
    cst::{CstInputValue, CstRootNode},
};
use serde::{Deserialize, Serialize};

use super::{McpError, Result};

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

fn enabled_by_default() -> bool {
    true
}

fn normalize_optional(value: &mut Option<String>) {
    *value = value
        .take()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
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

    fn disabled_tools_mut(&mut self) -> &mut BTreeSet<String> {
        match self {
            Self::Http(server) => &mut server.disabled_tools,
            Self::Stdio(server) => &mut server.disabled_tools,
        }
    }
}

impl McpConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path).map_err(|error| {
            McpError::new(format!("Could not read {}: {error}", path.display()))
        })?;
        Self::parse(&source)
            .map_err(|error| McpError::new(format!("Could not parse {}: {error}", path.display())))
    }

    pub fn parse(source: &str) -> Result<Self> {
        let mut config: Self = json5::from_str(source).map_err(McpError::from_display)?;
        config.normalize_and_validate()?;
        Ok(config)
    }

    pub(crate) fn validate_server(id: &str, server: &mut McpServerConfig) -> Result<()> {
        if id.trim().is_empty() {
            return Err(McpError::new("MCP server id cannot be empty"));
        }
        match server {
            McpServerConfig::Http(server) => {
                server.url = server.url.trim().to_string();
                let url = reqwest::Url::parse(&server.url).map_err(|error| {
                    McpError::new(format!("MCP server {id} has an invalid URL: {error}"))
                })?;
                if !matches!(url.scheme(), "http" | "https") {
                    return Err(McpError::new(format!(
                        "MCP server {id} URL must use http or https"
                    )));
                }
                let mut header_names = BTreeSet::new();
                for (name, value) in &server.headers {
                    reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                        McpError::new(format!(
                            "MCP server {id} has an invalid HTTP header name {name:?}: {error}"
                        ))
                    })?;
                    reqwest::header::HeaderValue::from_str(value).map_err(|error| {
                        McpError::new(format!(
                            "MCP server {id} has an invalid value for HTTP header {name:?}: {error}"
                        ))
                    })?;
                    if !header_names.insert(name.to_ascii_lowercase()) {
                        return Err(McpError::new(format!(
                            "MCP server {id} contains a duplicate HTTP header: {name}"
                        )));
                    }
                }
                normalize_optional(&mut server.proxy);
                if let Some(proxy) = &server.proxy {
                    reqwest::Proxy::all(proxy.as_str()).map_err(|error| {
                        McpError::new(format!("MCP server {id} has an invalid proxy: {error}"))
                    })?;
                }
                normalize_optional(&mut server.bearer_token);
                let has_authorization_header = server
                    .headers
                    .keys()
                    .any(|name| name.eq_ignore_ascii_case("authorization"));
                let auth_count = usize::from(server.bearer_token.is_some())
                    + usize::from(server.oauth.is_some())
                    + usize::from(has_authorization_header);
                if auth_count > 1 {
                    return Err(McpError::new(format!(
                        "MCP server {id} must use only one of oauth, bearerToken, or an Authorization header"
                    )));
                }
                if let Some(oauth) = &mut server.oauth {
                    normalize_optional(&mut oauth.client_id);
                    normalize_optional(&mut oauth.client_secret);
                    oauth.scopes = oauth
                        .scopes
                        .iter()
                        .map(|scope| scope.trim().to_string())
                        .filter(|scope| !scope.is_empty())
                        .collect();
                    if oauth.flow == McpOAuthFlow::ClientCredentials
                        && (oauth.client_id.is_none() || oauth.client_secret.is_none())
                    {
                        return Err(McpError::new(format!(
                            "MCP server {id} client credentials OAuth requires clientId and clientSecret"
                        )));
                    }
                }
            }
            McpServerConfig::Stdio(server) => {
                server.command = server.command.trim().to_string();
                if server.command.is_empty() {
                    return Err(McpError::new(format!("MCP server {id} requires a command")));
                }
                if let Some(cwd) = &server.cwd
                    && !cwd.is_absolute()
                {
                    return Err(McpError::new(format!(
                        "MCP server {id} cwd must be an absolute path"
                    )));
                }
                if let Some(name) = server
                    .env
                    .keys()
                    .find(|name| name.is_empty() || name.contains('='))
                {
                    return Err(McpError::new(format!(
                        "MCP server {id} has an invalid environment variable name: {name}"
                    )));
                }
            }
        }
        if server.disabled_tools().iter().any(|tool| tool.is_empty()) {
            return Err(McpError::new(format!(
                "MCP server {id} contains an empty disabled tool name"
            )));
        }
        Ok(())
    }

    fn normalize_and_validate(&mut self) -> Result<()> {
        for (id, server) in &mut self.servers {
            Self::validate_server(id, server)?;
        }
        Ok(())
    }

    pub(crate) fn upsert(path: &Path, id: &str, mut server: McpServerConfig) -> Result<()> {
        Self::validate_server(id, &mut server)?;
        edit_file(path, |servers| upsert_server(servers, id, &server))
    }

    pub(crate) fn import(path: &Path, source: &str) -> Result<usize> {
        let config = Self::parse(source)?;
        if config.servers.is_empty() {
            return Err(McpError::new("Imported JSON contains no MCP servers"));
        }
        let count = config.servers.len();
        edit_file(path, |servers| {
            for (id, server) in &config.servers {
                upsert_server(servers, id, server)?;
            }
            Ok(())
        })?;
        Ok(count)
    }

    pub(crate) fn delete(path: &Path, id: &str) -> Result<()> {
        edit_file(path, |servers| {
            let property = servers
                .get(id)
                .ok_or_else(|| McpError::new(format!("MCP server not found: {id}")))?;
            property.remove();
            Ok(())
        })
    }

    pub(crate) fn set_server_enabled(path: &Path, id: &str, enabled: bool) -> Result<()> {
        let mut config = Self::load(path)?;
        let server = config
            .servers
            .get_mut(id)
            .ok_or_else(|| McpError::new(format!("MCP server not found: {id}")))?;
        server.set_enabled(enabled);
        edit_file(path, |servers| upsert_server(servers, id, server))
    }

    pub(crate) fn set_tool_enabled(path: &Path, id: &str, tool: &str, enabled: bool) -> Result<()> {
        let mut config = Self::load(path)?;
        let server = config
            .servers
            .get_mut(id)
            .ok_or_else(|| McpError::new(format!("MCP server not found: {id}")))?;
        if enabled {
            server.disabled_tools_mut().remove(tool);
        } else {
            server.disabled_tools_mut().insert(tool.to_string());
        }
        edit_file(path, |servers| upsert_server(servers, id, server))
    }
}

fn edit_file(
    path: &Path,
    edit: impl FnOnce(&jsonc_parser::cst::CstObject) -> Result<()>,
) -> Result<()> {
    let source = fs::read_to_string(path)
        .map_err(|error| McpError::new(format!("Could not read {}: {error}", path.display())))?;
    McpConfig::parse(&source)?;
    let root = CstRootNode::parse(&source, &ParseOptions::default())
        .map_err(|error| McpError::new(format!("Could not parse {}: {error}", path.display())))?;
    let object = root
        .object_value()
        .ok_or_else(|| McpError::new("MCP config root must be an object"))?;
    let servers = object
        .object_value_or_create("mcpServers")
        .ok_or_else(|| McpError::new("mcpServers must be an object"))?;
    edit(&servers)?;

    let output = root.to_string();
    McpConfig::parse(&output)?;
    fs::write(path, output)
        .map_err(|error| McpError::new(format!("Could not write {}: {error}", path.display())))
}

fn upsert_server(
    servers: &jsonc_parser::cst::CstObject,
    id: &str,
    server: &McpServerConfig,
) -> Result<()> {
    if let Some(property) = servers.get(id) {
        let object = property
            .object_value()
            .ok_or_else(|| McpError::new(format!("MCP server {id} must be an object")))?;
        match server {
            McpServerConfig::Http(server) => {
                set_property(&object, "enabled", server.enabled);
                set_property(&object, "url", server.url.clone());
                set_string_map_property(&object, "headers", &server.headers)?;
                set_optional_string_property(&object, "proxy", server.proxy.as_deref());
                set_optional_string_property(
                    &object,
                    "bearerToken",
                    server.bearer_token.as_deref(),
                );
                set_optional_object_property(&object, "oauth", server.oauth.as_ref())?;
                set_string_set_property(&object, "disabledTools", &server.disabled_tools);
                remove_properties(&object, &["command", "args", "env", "cwd"]);
            }
            McpServerConfig::Stdio(server) => {
                set_property(&object, "enabled", server.enabled);
                set_property(&object, "command", server.command.clone());
                set_array_property(&object, "args", &server.args);
                set_string_map_property(&object, "env", &server.env)?;
                set_string_set_property(&object, "disabledTools", &server.disabled_tools);
                set_property(
                    &object,
                    "cwd",
                    server.cwd.as_ref().map_or(CstInputValue::Null, |path| {
                        path.to_string_lossy().into_owned().into()
                    }),
                );
                remove_properties(
                    &object,
                    &["url", "headers", "proxy", "bearerToken", "oauth"],
                );
            }
        }
    } else {
        servers.append(id, server_value(server));
    }
    Ok(())
}

fn remove_properties(object: &jsonc_parser::cst::CstObject, names: &[&str]) {
    for name in names {
        if let Some(property) = object.get(name) {
            property.remove();
        }
    }
}

fn set_string_set_property(
    object: &jsonc_parser::cst::CstObject,
    name: &str,
    values: &BTreeSet<String>,
) {
    set_array_property(object, name, &values.iter().cloned().collect::<Vec<_>>());
}

fn set_array_property(object: &jsonc_parser::cst::CstObject, name: &str, values: &[String]) {
    let array = if let Some(property) = object.get(name) {
        property.array_value_or_set()
    } else {
        let property = object.append(name, CstInputValue::Array(Vec::new()));
        property.array_value().expect("new array property")
    };
    let elements = array.elements();
    for (element, value) in elements.iter().zip(values) {
        let literal = element
            .as_string_lit()
            .expect("validated MCP arguments are strings");
        literal.set_raw_value(serde_json::to_string(value).expect("string serialization"));
    }
    for value in values.iter().skip(elements.len()) {
        array.append(value.clone().into());
    }
    for element in elements.into_iter().skip(values.len()).rev() {
        element
            .as_string_lit()
            .expect("validated MCP arguments are strings")
            .remove();
    }
}

fn set_optional_string_property(
    object: &jsonc_parser::cst::CstObject,
    name: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        set_property(object, name, value.to_string());
    } else if let Some(property) = object.get(name) {
        property.remove();
    }
}

fn set_optional_object_property<T: Serialize>(
    object: &jsonc_parser::cst::CstObject,
    name: &str,
    value: Option<&T>,
) -> Result<()> {
    if let Some(value) = value {
        let value = serde_json::to_value(value).map_err(McpError::from_display)?;
        set_property(object, name, json_value_to_cst(value)?);
    } else if let Some(property) = object.get(name) {
        property.remove();
    }
    Ok(())
}

fn json_value_to_cst(value: serde_json::Value) -> Result<CstInputValue> {
    match value {
        serde_json::Value::Null => Ok(CstInputValue::Null),
        serde_json::Value::Bool(value) => Ok(value.into()),
        serde_json::Value::Number(value) => value
            .as_u64()
            .map(CstInputValue::from)
            .or_else(|| value.as_i64().map(CstInputValue::from))
            .ok_or_else(|| McpError::new("MCP config number is unsupported")),
        serde_json::Value::String(value) => Ok(value.into()),
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(json_value_to_cst)
            .collect::<Result<Vec<_>>>()
            .map(CstInputValue::Array),
        serde_json::Value::Object(values) => values
            .into_iter()
            .map(|(name, value)| Ok((name, json_value_to_cst(value)?)))
            .collect::<Result<Vec<_>>>()
            .map(CstInputValue::Object),
    }
}

fn set_string_map_property(
    object: &jsonc_parser::cst::CstObject,
    name: &str,
    values: &BTreeMap<String, String>,
) -> Result<()> {
    let map = if let Some(property) = object.get(name) {
        property.object_value_or_set()
    } else {
        let property = object.append(name, CstInputValue::Object(Vec::new()));
        property.object_value().expect("new object property")
    };
    for (name, value) in values {
        set_property(&map, name, value.clone());
    }
    for property in map.properties() {
        let property_name = property
            .name()
            .ok_or_else(|| McpError::new(format!("MCP {name} property name is missing")))?
            .decoded_value()
            .map_err(McpError::from_display)?;
        if !values.contains_key(&property_name) {
            property.remove();
        }
    }
    Ok(())
}

fn set_property(
    object: &jsonc_parser::cst::CstObject,
    name: &str,
    value: impl Into<CstInputValue>,
) {
    let value = value.into();
    if let Some(property) = object.get(name) {
        property.set_value(value);
    } else {
        object.append(name, value);
    }
}

fn server_value(server: &McpServerConfig) -> CstInputValue {
    match server {
        McpServerConfig::Http(server) => {
            let mut values = vec![
                ("enabled".into(), server.enabled.into()),
                ("url".into(), server.url.clone().into()),
                (
                    "headers".into(),
                    CstInputValue::Object(
                        server
                            .headers
                            .iter()
                            .map(|(name, value)| (name.clone(), value.clone().into()))
                            .collect(),
                    ),
                ),
            ];
            if let Some(proxy) = &server.proxy {
                values.push(("proxy".into(), proxy.clone().into()));
            }
            if let Some(token) = &server.bearer_token {
                values.push(("bearerToken".into(), token.clone().into()));
            }
            if let Some(oauth) = &server.oauth {
                values.push((
                    "oauth".into(),
                    json_value_to_cst(serde_json::to_value(oauth).expect("OAuth serialization"))
                        .expect("OAuth config uses supported JSON values"),
                ));
            }
            values.push((
                "disabledTools".into(),
                server
                    .disabled_tools
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ));
            CstInputValue::Object(values)
        }
        McpServerConfig::Stdio(server) => CstInputValue::Object(vec![
            ("enabled".into(), server.enabled.into()),
            ("command".into(), server.command.clone().into()),
            ("args".into(), server.args.clone().into()),
            (
                "env".into(),
                CstInputValue::Object(
                    server
                        .env
                        .iter()
                        .map(|(name, value)| (name.clone(), value.clone().into()))
                        .collect(),
                ),
            ),
            (
                "cwd".into(),
                server.cwd.as_ref().map_or(CstInputValue::Null, |path| {
                    path.to_string_lossy().into_owned().into()
                }),
            ),
            (
                "disabledTools".into(),
                server
                    .disabled_tools
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ),
        ]),
    }
}
