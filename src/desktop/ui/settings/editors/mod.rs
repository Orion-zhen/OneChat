mod catalog;
mod items;
mod mcp;
mod prompt;
mod prompt_variable;

use super::*;

pub(crate) use catalog::{Capability, ModelEditor, ModelFetchStatus, ProviderEditor};
pub(crate) use items::{
    DefaultModelItem, FontFamilyItem, ModelIdDelegate, PromptSelectItem, ProviderKindItem,
    ReasoningPresetSelectItem, SearchableItems, SettingsSection, font_family_label,
};
pub(crate) use mcp::{McpServerEditor, McpServerEditorMode, McpServerTransportEditor};
pub(crate) use prompt::PromptPresetEditor;
pub(crate) use prompt_variable::{
    PromptVariableEditor, PromptVariableKind, PromptVariableTestStatus,
};

pub(crate) struct KeyValueEditor {
    pub(crate) name: Entity<InputState>,
    pub(crate) value: Entity<InputState>,
}

impl KeyValueEditor {
    fn new(
        name: impl Into<String>,
        value: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        Self {
            name: single_line_input(name, "Name", window, cx),
            value: single_line_input(value, "Value", window, cx),
        }
    }
}

fn append_draft_if_tail_complete<T>(
    items: &mut Vec<T>,
    tail_complete: bool,
    draft: impl FnOnce() -> T,
) {
    if tail_complete {
        items.push(draft());
    }
}

fn remove_committed<T>(items: &mut Vec<T>, index: usize) {
    if index < items.len().saturating_sub(1) {
        items.remove(index);
    }
}

fn single_line_input(
    value: impl Into<String>,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(value.into())
            .placeholder(placeholder)
    })
}

fn masked_input(
    value: impl Into<String>,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(value.into())
            .placeholder(placeholder)
            .masked(true)
    })
}

#[cfg(test)]
mod collection_tests {
    use super::{append_draft_if_tail_complete, remove_committed};

    #[test]
    fn appends_only_after_complete_tail() {
        let mut items = vec!["saved", ""];
        append_draft_if_tail_complete(&mut items, false, || "new");
        assert_eq!(items, ["saved", ""]);

        items[1] = "complete";
        append_draft_if_tail_complete(&mut items, true, || "");
        assert_eq!(items, ["saved", "complete", ""]);
    }

    #[test]
    fn removes_only_committed_rows() {
        let mut items = vec!["first", "second", ""];
        remove_committed(&mut items, 1);
        assert_eq!(items, ["first", ""]);

        remove_committed(&mut items, 1);
        assert_eq!(items, ["first", ""]);
    }
}
