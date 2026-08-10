use super::*;

impl McpConfig {
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

    pub(super) fn normalize_and_validate(&mut self) -> Result<()> {
        for (id, server) in &mut self.servers {
            Self::validate_server(id, server)?;
        }
        Ok(())
    }
}
