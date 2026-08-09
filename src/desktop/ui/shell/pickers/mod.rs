use gpui::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, SharedString, Task, Window, div,
    prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, IndexPath, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::{CancelDialog, Dialog, DialogFooter},
    list::{List, ListDelegate, ListItem, ListState},
};

use crate::{
    desktop::{
        app::{OneChat, PaletteCommand},
        ui::inspector,
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

pub(crate) fn command_palette_dialog(
    dialog: Dialog,
    list: Entity<ListState<CommandPaletteDelegate>>,
    cx: &App,
) -> Dialog {
    let row_count = list.read(cx).delegate().row_count();
    picker_dialog(
        dialog,
        640.0,
        "Command Palette",
        "Jump to an action without leaving the keyboard.",
        cx,
    )
    .child(picker_list(
        &list,
        "Type a command…",
        picker_height(row_count as f32 * 56.0),
        cx,
    ))
    .footer(picker_help(cx))
}

pub(crate) fn model_picker_dialog(
    dialog: Dialog,
    list: Entity<ListState<ModelPickerDelegate>>,
    adding_response: bool,
    cx: &App,
) -> Dialog {
    let content_height = list.read(cx).delegate().content_height();
    picker_dialog(
        dialog,
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
        cx,
    )
    .child(picker_list(
        &list,
        "Search models…",
        picker_height(content_height),
        cx,
    ))
    .footer(picker_help(cx))
}

pub(crate) fn reasoning_picker_dialog(
    dialog: Dialog,
    list: Entity<ListState<ReasoningPickerDelegate>>,
    cx: &App,
) -> Dialog {
    let row_count = list.read(cx).delegate().row_count();
    picker_dialog(
        dialog,
        480.0,
        "Choose Reasoning",
        "Select the reasoning preset for the next response.",
        cx,
    )
    .child(picker_list(
        &list,
        "Search presets…",
        picker_height(row_count as f32 * 56.0),
        cx,
    ))
    .footer(picker_help(cx))
}

pub(crate) fn prompt_picker_dialog(
    dialog: Dialog,
    list: Entity<ListState<PromptPickerDelegate>>,
    app: Entity<OneChat>,
    cx: &App,
) -> Dialog {
    let row_count = list.read(cx).delegate().row_count();
    picker_dialog(
        dialog,
        640.0,
        "Choose System Prompt",
        "Apply reusable instructions to this conversation.",
        cx,
    )
    .child(picker_list(
        &list,
        "Search prompts…",
        picker_height(row_count as f32 * 72.0),
        cx,
    ))
    .footer(
        DialogFooter::new()
            .justify_between()
            .pt_1()
            .child(
                Button::new("manage-prompt-presets")
                    .ghost()
                    .tooltip("Manage prompts")
                    .size(px(36.0))
                    .p_0()
                    .icon(IconName::Settings2)
                    .on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        app.update(cx, |app, cx| app.open_prompt_settings(cx));
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(key_hint(&["↩"], "Select", cx))
                    .child(key_hint(&["esc"], "Close", cx)),
            ),
    )
}

fn picker_dialog(
    dialog: Dialog,
    width: f32,
    title: &'static str,
    subtitle: &'static str,
    cx: &App,
) -> Dialog {
    dialog
        .width(px(width))
        .margin_top(px(56.0))
        .p(px(22.0))
        .rounded(px(22.0))
        .border_color(cx.theme().border)
        .bg(cx.theme().popover.alpha(0.95))
        .shadow_xl()
        .close_button(false)
        .title(
            div()
                .relative()
                .w_full()
                .pr(px(40.0))
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
                )
                .child(
                    Button::new("close-picker-dialog")
                        .absolute()
                        .top(px(0.0))
                        .right(px(0.0))
                        .ghost()
                        .tooltip("Close")
                        .size(px(36.0))
                        .p_0()
                        .rounded(px(11.0))
                        .child(Icon::new(IconName::Close).size(px(18.0)))
                        .on_click(|_, window, cx| {
                            window.dispatch_action(Box::new(CancelDialog), cx)
                        }),
                ),
        )
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

fn picker_help(cx: &App) -> impl IntoElement {
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

fn prompt_excerpt(prompt: &str) -> String {
    const MAX_CHARACTERS: usize = 100;
    let prompt = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = prompt.chars();
    let excerpt = characters.by_ref().take(MAX_CHARACTERS).collect::<String>();
    if characters.next().is_some() {
        format!("{excerpt}…")
    } else if excerpt.is_empty() {
        "Empty prompt".into()
    } else {
        excerpt
    }
}
