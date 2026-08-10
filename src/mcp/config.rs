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

mod edit;
mod model;
mod validation;

pub use model::{
    McpConfig, McpHttpServerConfig, McpOAuthConfig, McpOAuthFlow, McpServerConfig,
    McpStdioServerConfig,
};

fn enabled_by_default() -> bool {
    true
}

fn normalize_optional(value: &mut Option<String>) {
    *value = value
        .take()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
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
}
