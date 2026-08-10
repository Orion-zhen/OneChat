mod general;
mod mcp;
mod prompts;

pub(super) use general::{default_models_page, general_page};
pub(super) use mcp::mcp_page;
pub(super) use prompts::{
    prompt_preset_dialog_body, prompt_variable_dialog_body, system_prompts_page,
};
