use std::{collections::BTreeMap, path::PathBuf};

use serde_json::Value;

use super::executable::ExecutionEnvironment;

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

pub(super) fn executable_snapshots(
    environment: &ExecutionEnvironment,
) -> Vec<McpExecutableSnapshot> {
    ["npx", "uv", "docker"]
        .into_iter()
        .map(|name| McpExecutableSnapshot {
            name: name.to_string(),
            path: environment.resolve(name, None, &BTreeMap::new()),
        })
        .collect()
}
