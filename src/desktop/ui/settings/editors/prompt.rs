use super::*;

pub struct PromptPresetEditor {
    original_name: Option<String>,
    pub name: Entity<InputState>,
    pub content: Entity<TextareaState>,
}

impl PromptPresetEditor {
    pub fn new(
        preset: Option<SystemPromptPreset>,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        let original_name = preset.as_ref().map(|preset| preset.name.clone());
        let preset = preset.unwrap_or_else(|| SystemPromptPreset::new("", ""));
        Self {
            original_name,
            name: single_line_input(preset.name, "Preset name", window, cx),
            content: multiline_input(
                preset.content,
                "Describe how the assistant should respond",
                window,
                cx,
            ),
        }
    }

    pub fn original_name(&self) -> Option<&str> {
        self.original_name.as_deref()
    }

    pub fn focus_handle(&self, cx: &App) -> gpui::FocusHandle {
        if self.original_name.is_some() {
            self.content.read(cx).focus_handle(cx)
        } else {
            self.name.read(cx).focus_handle(cx)
        }
    }

    pub fn build(&self, cx: &App) -> Result<SystemPromptPreset, String> {
        let preset = SystemPromptPreset::new(
            self.name.read(cx).value().to_string(),
            self.content.read(cx).value().to_string(),
        );
        if preset.name.is_empty() {
            return Err("Prompt preset name is required.".into());
        }
        if preset.name.starts_with('.') || preset.name.contains('/') {
            return Err("Prompt preset name cannot start with a dot or contain a slash.".into());
        }
        if preset.content.is_empty() {
            return Err("Prompt preset content is required.".into());
        }
        Ok(preset)
    }
}
