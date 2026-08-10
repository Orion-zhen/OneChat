use gpui::{Context, Window};

use crate::{
    desktop::{
        app::{ConnectionTestStatus, DestructiveAction, OneChat},
        ui::settings::{McpServerEditor, McpServerEditorMode, McpServerTransportEditor},
    },
    mcp::{McpConfig, McpServerConfig},
};

mod connection;
mod editor;
