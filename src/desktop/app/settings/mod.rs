mod appearance;
mod catalog;
mod mcp;
mod prompts;

use gpui::Context;

use super::OneChat;
use crate::desktop::ui::settings::SettingsSection;

impl OneChat {
    pub(crate) fn select_settings_section(
        &mut self,
        section: SettingsSection,
        cx: &mut Context<Self>,
    ) {
        let reload_prompts = section == SettingsSection::SystemPrompts;
        if self.settings_ui.section == section {
            if reload_prompts {
                self.reload_snapshot(cx);
            }
            return;
        }
        self.settings_ui.section = section;
        self.settings_ui.viewed_prompt_preset = None;
        self.settings_ui.provider_editor = None;
        self.settings_ui.model_editor = None;
        self.settings_ui.prompt_preset_editor = None;
        self.settings_ui.title_prompt_editor = None;
        self.settings_ui.mcp_server_editor = None;
        self.settings_ui.mcp_error = None;
        self.settings_ui.form_error = None;
        if reload_prompts {
            self.reload_snapshot(cx);
        }
        cx.notify();
    }
}
