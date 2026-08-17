use gpui::{
    AnyElement, App, Context, Entity, Focusable as _, FontWeight, IntoElement, MouseButton,
    SharedString, Task, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, IndexPath, Sizable as _,
    button::{Button, ButtonVariants as _},
    dialog::DialogFooter,
    list::{List, ListDelegate, ListItem, ListState},
};

use crate::{
    desktop::{
        app::{OneChat, PaletteCommand, ShellOverlay},
        ui::{
            icons::selected_check_badge, model::capability_summary as model_capability_summary,
            text::summary as text_summary,
        },
    },
    domain::Model,
};

mod command;
mod model;
mod prompt;
mod reasoning;

pub(crate) use command::CommandPaletteDelegate;
pub(crate) use model::ModelPickerDelegate;
pub(crate) use prompt::PromptPickerDelegate;
pub(crate) use reasoning::ReasoningPickerDelegate;

#[derive(Clone)]
struct FlatPickerState<T> {
    all: Vec<T>,
    filtered: Vec<T>,
    selected: Option<IndexPath>,
}

impl<T: Clone> FlatPickerState<T> {
    fn empty() -> Self {
        Self {
            all: Vec::new(),
            filtered: Vec::new(),
            selected: None,
        }
    }

    fn new(all: Vec<T>, is_selected: impl Fn(&T) -> bool) -> Self {
        let selected = all
            .iter()
            .position(is_selected)
            .map(IndexPath::new)
            .or((!all.is_empty()).then(IndexPath::default));
        Self {
            filtered: all.clone(),
            all,
            selected,
        }
    }

    fn filter(&mut self, mut matches: impl FnMut(&T) -> bool) {
        self.filtered = self
            .all
            .iter()
            .filter(|item| matches(item))
            .cloned()
            .collect();
    }

    fn len(&self) -> usize {
        self.filtered.len()
    }

    fn get(&self, index: IndexPath) -> Option<&T> {
        self.filtered.get(index.row)
    }

    fn initial_selection(&self) -> Option<IndexPath> {
        self.selected
    }

    fn selected(&self) -> Option<IndexPath> {
        self.selected
    }

    fn set_selected(&mut self, selected: Option<IndexPath>) {
        self.selected = selected;
    }
}

pub(crate) fn render_picker_overlay(
    app: &OneChat,
    picker: ShellOverlay,
    progress: f32,
    reduce_motion: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let (width, title, subtitle, body, footer, focus) = match picker {
        ShellOverlay::CommandPalette => {
            let list = app.overlays.command_picker.clone();
            let row_count = list.read(cx).delegate().row_count();
            (
                640.0,
                "Command Palette",
                "Jump to an action without leaving the keyboard.",
                picker_list(
                    &list,
                    "Type a command…",
                    picker_height(row_count as f32 * 56.0),
                    cx,
                )
                .into_any_element(),
                picker_help(cx).into_any_element(),
                list.read(cx).focus_handle(cx),
            )
        }
        ShellOverlay::ConversationSearch => unreachable!("search has a dedicated overlay"),
        ShellOverlay::ModelPicker => {
            let list = app.overlays.model_picker.clone();
            let adding_response = app.overlays.response_model_turn_id.is_some();
            let content_height = list.read(cx).delegate().content_height();
            (
                680.0,
                if adding_response {
                    "Choose another model"
                } else {
                    "Choose Model"
                },
                if adding_response {
                    "Add a parallel response from a different model."
                } else {
                    "Select the model for the next response."
                },
                picker_list(&list, "Search models…", picker_height(content_height), cx)
                    .into_any_element(),
                picker_help(cx).into_any_element(),
                list.read(cx).focus_handle(cx),
            )
        }
        ShellOverlay::ReasoningPicker => {
            let list = app.overlays.reasoning_picker.clone();
            let row_count = list.read(cx).delegate().row_count();
            (
                480.0,
                "Choose Reasoning",
                "Select the reasoning preset for the next response.",
                picker_list(
                    &list,
                    "Search presets…",
                    picker_height(row_count as f32 * 56.0),
                    cx,
                )
                .into_any_element(),
                picker_help(cx).into_any_element(),
                list.read(cx).focus_handle(cx),
            )
        }
        ShellOverlay::PromptPicker => {
            let list = app.overlays.prompt_picker.clone();
            let row_count = list.read(cx).delegate().row_count();
            let footer = DialogFooter::new()
                .justify_between()
                .pt_1()
                .child(
                    Button::new("manage-prompt-presets")
                        .ghost()
                        .tooltip("Manage prompts")
                        .size(px(36.0))
                        .p_0()
                        .icon(IconName::Settings2)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.close_shell_overlay(true, cx);
                            this.open_prompt_settings(cx);
                        })),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .child(key_hint(&["↩"], "Select", cx))
                        .child(key_hint(&["esc"], "Close", cx)),
                );
            (
                640.0,
                "Choose Prompt Preset",
                "Apply a system prompt and optional assistant opening to this conversation.",
                picker_list(
                    &list,
                    "Search prompts…",
                    picker_height(row_count as f32 * 72.0),
                    cx,
                )
                .into_any_element(),
                footer.into_any_element(),
                list.read(cx).focus_handle(cx),
            )
        }
    };

    let panel =
        super::floating_overlay::panel("picker-overlay-panel", title, &focus, width, 22.0, cx)
            .gap_2()
            .child(picker_header(
                title,
                subtitle,
                Button::new("close-picker-overlay")
                    .ghost()
                    .tooltip("Close")
                    .size(px(36.0))
                    .p_0()
                    .rounded(px(11.0))
                    .child(Icon::new(IconName::Close).size(px(18.0)))
                    .on_click(cx.listener(|this, _, _, cx| this.close_shell_overlay(true, cx))),
                cx,
            ))
            .child(body)
            .child(footer);
    super::floating_overlay::backdrop("picker-overlay", panel, progress, reduce_motion, cx)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| this.close_shell_overlay(true, cx)),
        )
        .into_any_element()
}

fn picker_header(
    title: &'static str,
    subtitle: &'static str,
    close: impl IntoElement,
    cx: &App,
) -> gpui::Div {
    div()
        .w_full()
        .flex()
        .items_start()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .pr(px(4.0))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(19.0))
                        .line_height(px(24.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(cx.theme().muted_foreground)
                        .child(subtitle),
                ),
        )
        .child(div().flex_none().child(close))
}

fn picker_list<D: ListDelegate + 'static>(
    list: &Entity<ListState<D>>,
    placeholder: &'static str,
    height: f32,
    cx: &App,
) -> List<D> {
    List::new(list)
        .large()
        .search_placeholder(placeholder)
        .h(px(height))
        .min_h_0()
        .w_full()
        .p_2()
        .rounded(px(14.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .overflow_hidden()
        .scrollbar_visible(false)
}

fn picker_height(content_height: f32) -> f32 {
    (61.0 + content_height).clamp(180.0, 420.0)
}

pub(super) fn picker_help(cx: &App) -> impl IntoElement {
    DialogFooter::new()
        .justify_between()
        .pt_1()
        .child(
            div()
                .flex()
                .items_center()
                .gap_4()
                .child(key_hint(&["↑", "↓"], "Navigate", cx))
                .child(key_hint(&["↩"], "Select", cx)),
        )
        .child(key_hint(&["esc"], "Close", cx))
}

fn key_hint(keys: &[&'static str], label: &'static str, cx: &App) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        .children(keys.iter().map(|key| key_cap(*key, cx)))
        .child(
            div()
                .ml_1()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .into_any_element()
}

fn key_cap(label: impl Into<SharedString>, cx: &App) -> AnyElement {
    div()
        .min_w(px(22.0))
        .h(px(20.0))
        .px_1()
        .rounded(px(6.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(label.into())
        .into_any_element()
}

fn empty_notice(message: &'static str, cx: &App) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p_5()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(message)
        .into_any_element()
}

fn command_shortcut(command: PaletteCommand) -> Option<String> {
    match command {
        PaletteCommand::NewConversation => Some(super::shortcut_label("N")),
        PaletteCommand::ChooseModel => Some(super::shortcut_label("L")),
        PaletteCommand::ToggleSidebar => Some(if cfg!(target_os = "macos") {
            "⇧⌘S".into()
        } else {
            "Ctrl+Shift+S".into()
        }),
        PaletteCommand::OpenSettings => Some(super::shortcut_label(",")),
        _ => None,
    }
}

#[cfg(test)]
mod flat_picker_tests {
    use super::FlatPickerState;
    use gpui_component::IndexPath;

    #[test]
    fn filters_without_discarding_all_items_or_selection() {
        let mut state = FlatPickerState::new(vec!["first", "selected", "last"], |item| {
            *item == "selected"
        });
        state.filter(|item| item.contains('s'));

        assert_eq!(state.len(), 3);
        assert_eq!(state.initial_selection(), Some(IndexPath::new(1)));
        state.filter(|item| item.starts_with('l'));
        assert_eq!(state.get(IndexPath::default()), Some(&"last"));
        assert_eq!(state.initial_selection(), Some(IndexPath::new(1)));
    }

    #[test]
    fn defaults_to_first_item_and_handles_empty_state() {
        let state = FlatPickerState::new(vec![1, 2], |_| false);
        assert_eq!(state.initial_selection(), Some(IndexPath::default()));
        assert_eq!(FlatPickerState::<i32>::empty().initial_selection(), None);
    }
}
