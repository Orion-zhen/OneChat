use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum PromptPresetSection {
    #[default]
    SystemPrompt,
    AssistantOpening,
}

impl PromptPresetSection {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::SystemPrompt => "System Prompt",
            Self::AssistantOpening => "Assistant Opening",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PromptPresetWorkspaceMode {
    View,
    Edit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PromptPresetDraft {
    name: String,
    system_prompt: String,
    assistant_opening: String,
}

pub struct PromptPresetEditor {
    original_name: Option<String>,
    baseline: PromptPresetDraft,
    pub name: Entity<InputState>,
    pub system_prompt: Entity<TextareaState>,
    pub assistant_opening: Entity<TextareaState>,
}

impl PromptPresetEditor {
    pub fn new(
        preset: Option<PromptPreset>,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        let original_name = preset.as_ref().map(|preset| preset.name.clone());
        let preset = preset.unwrap_or_else(|| PromptPreset::new("", "", ""));
        let baseline = PromptPresetDraft {
            name: preset.name.clone(),
            system_prompt: preset.system_prompt.clone(),
            assistant_opening: preset.assistant_opening.clone(),
        };
        Self {
            original_name,
            baseline,
            name: single_line_input(preset.name, "Preset name", window, cx),
            system_prompt: multiline_input(
                preset.system_prompt,
                "Describe how the assistant should respond",
                window,
                cx,
            ),
            assistant_opening: multiline_input(
                preset.assistant_opening,
                "Optional first assistant message",
                window,
                cx,
            ),
        }
    }

    pub fn original_name(&self) -> Option<&str> {
        self.original_name.as_deref()
    }

    fn draft(&self, cx: &App) -> PromptPresetDraft {
        PromptPresetDraft {
            name: self.name.read(cx).value().to_string(),
            system_prompt: self.system_prompt.read(cx).value().to_string(),
            assistant_opening: self.assistant_opening.read(cx).value().to_string(),
        }
    }

    pub fn is_dirty(&self, cx: &App) -> bool {
        self.draft(cx) != self.baseline
    }

    pub fn text(&self, section: PromptPresetSection, cx: &App) -> String {
        match section {
            PromptPresetSection::SystemPrompt => self.system_prompt.read(cx).value().to_string(),
            PromptPresetSection::AssistantOpening => {
                self.assistant_opening.read(cx).value().to_string()
            }
        }
    }

    pub fn input(&self, section: PromptPresetSection) -> Entity<TextareaState> {
        match section {
            PromptPresetSection::SystemPrompt => self.system_prompt.clone(),
            PromptPresetSection::AssistantOpening => self.assistant_opening.clone(),
        }
    }

    pub fn focus_handle(&self, section: PromptPresetSection, cx: &App) -> gpui::FocusHandle {
        self.input(section).read(cx).focus_handle(cx)
    }

    pub fn make_duplicate(&mut self, name: String, window: &mut Window, cx: &mut Context<OneChat>) {
        self.original_name = None;
        self.name
            .update(cx, |input, cx| input.set_value(name, window, cx));
    }

    pub fn build(&self, cx: &App) -> Result<PromptPreset, String> {
        let draft = self.draft(cx);
        let preset = PromptPreset::new(draft.name, draft.system_prompt, draft.assistant_opening);
        if preset.name.is_empty() {
            return Err("Prompt preset name is required.".into());
        }
        if preset.name.starts_with('.') || preset.name.contains('/') {
            return Err("Prompt preset name cannot start with a dot or contain a slash.".into());
        }
        if preset.system_prompt.is_empty() {
            return Err("System prompt is required.".into());
        }
        Ok(preset)
    }
}

pub(crate) struct PromptPresetWorkspace {
    pub(crate) editor: PromptPresetEditor,
    pub(crate) mode: PromptPresetWorkspaceMode,
    pub(crate) section: PromptPresetSection,
    pub(crate) inspector_open: bool,
    pub(crate) focus_mode: bool,
}

impl PromptPresetWorkspace {
    pub(crate) fn view(editor: PromptPresetEditor) -> Self {
        Self {
            editor,
            mode: PromptPresetWorkspaceMode::View,
            section: PromptPresetSection::SystemPrompt,
            inspector_open: false,
            focus_mode: false,
        }
    }

    pub(crate) fn edit(editor: PromptPresetEditor) -> Self {
        let inspector_open = editor.original_name().is_none();
        Self {
            editor,
            mode: PromptPresetWorkspaceMode::Edit,
            section: PromptPresetSection::SystemPrompt,
            inspector_open,
            focus_mode: false,
        }
    }

    pub(crate) fn is_editing(&self) -> bool {
        self.mode == PromptPresetWorkspaceMode::Edit
    }

    pub(crate) fn is_dirty(&self, cx: &App) -> bool {
        self.is_editing() && self.editor.is_dirty(cx)
    }
}
