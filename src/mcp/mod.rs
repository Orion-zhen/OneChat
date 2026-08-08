mod config;
mod executable;
mod manager;

use std::fmt::{self, Display, Formatter};

pub use config::{
    McpConfig, McpHttpServerConfig, McpOAuthConfig, McpOAuthFlow, McpServerConfig,
    McpStdioServerConfig,
};
pub use manager::{
    McpExecutableSnapshot, McpManager, McpServerSnapshot, McpServerStatus,
    McpServerTransportSnapshot, McpSnapshot, McpToolDefinition, McpToolSnapshot,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpError {
    message: String,
}

impl McpError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn from_display(error: impl Display) -> Self {
        Self::new(error.to_string())
    }
}

impl Display for McpError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for McpError {}

pub type Result<T> = std::result::Result<T, McpError>;
