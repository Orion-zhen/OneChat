mod catalog;
mod items;
mod mcp;
mod prompt;

use super::*;

pub(crate) use catalog::{Capability, ModelEditor, ModelFetchStatus, ProviderEditor};
pub(crate) use items::{
    DefaultModelItem, FontFamilyItem, ModelIdDelegate, PromptSelectItem, ProviderKindItem,
    SearchableItems, SettingsSection, font_family_label,
};
pub(crate) use mcp::{McpServerEditor, McpServerEditorMode, McpServerTransportEditor};
pub(crate) use prompt::PromptPresetEditor;

pub(crate) struct KeyValueEditor {
    pub name: Entity<InputState>,
    pub value: Entity<InputState>,
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

fn multiline_input(
    value: impl Into<String>,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .multi_line(true)
            .soft_wrap(true)
            .default_value(value.into())
            .placeholder(placeholder)
    })
}
