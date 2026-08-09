mod mcp;
mod model;
mod provider;

use super::*;

fn form_input(state: &Entity<InputState>, label: &'static str) -> Input {
    Input::new(state).large().max_h(px(40.0)).aria_label(label)
}

pub(super) use mcp::mcp_server_form;
pub(super) use model::model_form;
pub(super) use provider::provider_form;
