use gpui::Context;

use super::super::OneChat;
use crate::storage::{ModelCatalog, Storage};

impl OneChat {
    pub(in crate::desktop::app) fn apply_model_catalog(
        &mut self,
        catalog: ModelCatalog,
        _cx: &mut Context<Self>,
    ) {
        self.data.snapshot.apply_model_catalog(catalog);
        self.settings_ui.controls_dirty = true;
        self.chat.controls_dirty = true;
        self.data.error = None;
    }

    pub(in crate::desktop::app) fn load_prompt_presets(&mut self, cx: &mut Context<Self>) {
        self.spawn_storage(
            Storage::load_prompt_presets,
            |this, presets, _| {
                this.data.snapshot.prompt_presets = presets;
                this.settings_ui.controls_dirty = true;
                this.data.error = None;
            },
            cx,
        );
    }
}
