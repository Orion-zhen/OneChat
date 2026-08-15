use super::super::super::*;
mod inspector;

use inspector::{PromptMetrics, inspector, prompt_metrics};

pub(in crate::desktop::ui::settings) fn prompt_preset_workspace(
    app: &OneChat,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let workspace = app
        .settings_ui
        .prompt_preset_workspace
        .as_ref()
        .expect("prompt preset workspace requires state");
    let editor = &workspace.editor;
    let editing = workspace.is_editing();
    let dirty = workspace.is_dirty(cx);
    let section = workspace.section;
    let focus_mode = workspace.focus_mode;
    let inspector_open = workspace.inspector_open && !focus_mode;
    let name = editor.name.read(cx).value().to_string();
    let title = if name.trim().is_empty() {
        "New Prompt Preset".to_string()
    } else {
        name
    };
    let metrics = prompt_metrics(app, editor, section, cx);

    div()
        .id("prompt-preset-workspace")
        .size_full()
        .min_w_0()
        .flex()
        .flex_col()
        .bg(cx.theme().background)
        .child(workspace_toolbar(
            title,
            editing,
            dirty,
            focus_mode,
            inspector_open,
            cx,
        ))
        .children(
            app.settings_ui
                .form_error
                .as_deref()
                .map(|error| div().flex_none().px_4().pt_3().child(error_banner(error))),
        )
        .child(
            div()
                .min_h_0()
                .flex_1()
                .flex()
                .children((!focus_mode).then(|| section_rail(editor, section, cx)))
                .child(editor_canvas(editor, section, editing, &metrics, cx))
                .children(inspector_open.then(|| inspector(app, editor, editing, &metrics, cx))),
        )
        .into_any_element()
}

fn workspace_toolbar(
    title: String,
    editing: bool,
    dirty: bool,
    focus_mode: bool,
    inspector_open: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .h(px(54.0))
        .w_full()
        .flex_none()
        .px_3()
        .flex()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover.opacity(0.94))
        .child(
            div().w(px(168.0)).flex_none().child(
                Compact
                    .icon_action(
                        "close-prompt-preset-workspace",
                        AppIcon::ChevronLeft,
                        IconTone::Muted,
                        "Back to prompts",
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.request_close_prompt_preset_workspace(window, cx)
                    })),
            ),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    div()
                        .max_w(px(420.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .children((editing && dirty).then(|| {
                    div()
                        .size(px(6.0))
                        .flex_none()
                        .rounded_full()
                        .bg(cx.theme().accent)
                })),
        )
        .child(
            div()
                .w(px(168.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_end()
                .gap_1()
                .when(!editing, |actions| {
                    actions.child(
                        Compact
                            .icon_action(
                                "edit-viewed-prompt-preset",
                                AppIcon::Pencil,
                                IconTone::Muted,
                                "Edit preset",
                                cx,
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit_viewed_prompt_preset(window, cx)
                            })),
                    )
                })
                .child(
                    Compact
                        .icon_action(
                            "duplicate-prompt-preset",
                            AppIcon::Copy,
                            IconTone::Muted,
                            "Duplicate preset",
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.duplicate_prompt_preset(window, cx)
                        })),
                )
                .child(
                    Compact
                        .icon_action(
                            "toggle-prompt-focus-mode",
                            if focus_mode {
                                AppIcon::Minimize
                            } else {
                                AppIcon::Maximize
                            },
                            if focus_mode {
                                IconTone::Accent
                            } else {
                                IconTone::Muted
                            },
                            if focus_mode {
                                "Exit focus mode"
                            } else {
                                "Enter focus mode"
                            },
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.toggle_prompt_preset_focus_mode(window, cx)
                        })),
                )
                .child(
                    Compact
                        .icon_action(
                            "toggle-prompt-preset-inspector",
                            AppIcon::Info,
                            if inspector_open {
                                IconTone::Accent
                            } else {
                                IconTone::Muted
                            },
                            "Toggle inspector",
                            cx,
                        )
                        .disabled(focus_mode)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.toggle_prompt_preset_inspector(cx)),
                        ),
                )
                .children(editing.then(|| {
                    Compact
                        .primary_icon_action(
                            "save-prompt-preset",
                            AppIcon::Save,
                            if cfg!(target_os = "macos") {
                                "Save preset (⌘S)"
                            } else {
                                "Save preset (Ctrl+S)"
                            },
                            cx,
                        )
                        .disabled(!dirty)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.save_prompt_preset(cx);
                        }))
                })),
        )
        .into_any_element()
}

fn section_rail(
    editor: &PromptPresetEditor,
    selected: PromptPresetSection,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .w(px(184.0))
        .h_full()
        .flex_none()
        .border_r_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().sidebar)
        .px_3()
        .py_4()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .px_2()
                .pb_2()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child("SECTIONS"),
        )
        .child(section_row(
            "prompt-section-system",
            PromptPresetSection::SystemPrompt,
            editor.system_prompt.read(cx).value().chars().count(),
            selected,
            cx,
        ))
        .child(section_row(
            "prompt-section-opening",
            PromptPresetSection::AssistantOpening,
            editor.assistant_opening.read(cx).value().chars().count(),
            selected,
            cx,
        ))
        .into_any_element()
}

fn section_row(
    id: &'static str,
    section: PromptPresetSection,
    characters: usize,
    selected: PromptPresetSection,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .id(id)
        .w_full()
        .h(px(42.0))
        .rounded(px(9.0))
        .px_2()
        .flex()
        .items_center()
        .justify_between()
        .cursor_pointer()
        .when(section == selected, |row| row.bg(cx.theme().list_hover))
        .hover(|style| style.bg(cx.theme().list_hover))
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(13.0))
                .font_weight(if section == selected {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .child(section.title()),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child(characters.to_string()),
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            this.select_prompt_preset_section(section, window, cx)
        }))
        .into_any_element()
}

fn editor_canvas(
    editor: &PromptPresetEditor,
    section: PromptPresetSection,
    editing: bool,
    metrics: &PromptMetrics,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let input = editor.input(section);
    let detail = match section {
        PromptPresetSection::SystemPrompt => {
            "Required instructions sent at the start of the conversation."
        }
        PromptPresetSection::AssistantOpening => "Optional first message from the assistant.",
    };
    div()
        .min_w_0()
        .h_full()
        .flex_1()
        .flex()
        .flex_col()
        .child(
            div().flex_none().px_8().pt_7().pb_4().child(
                div()
                    .mx_auto()
                    .w_full()
                    .max_w(px(840.0))
                    .child(
                        div()
                            .text_size(px(20.0))
                            .line_height(px(26.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(section.title()),
                    )
                    .child(
                        div()
                            .pt_1()
                            .text_size(px(12.0))
                            .text_color(cx.theme().muted_foreground)
                            .child(detail),
                    ),
            ),
        )
        .child(
            div().min_h_0().flex_1().px_8().pb_4().child(
                div()
                    .mx_auto()
                    .w_full()
                    .h_full()
                    .max_w(px(840.0))
                    .rounded(px(14.0))
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().popover)
                    .overflow_hidden()
                    .child(
                        Textarea::new(&input)
                            .appearance(false)
                            .disabled(!editing)
                            .w_full()
                            .h_full()
                            .px_4()
                            .py_3()
                            .text_size(px(14.0))
                            .line_height(px(22.0))
                            .aria_label(section.title()),
                    ),
            ),
        )
        .child(
            div()
                .h(px(34.0))
                .flex_none()
                .border_t_1()
                .border_color(cx.theme().border)
                .px_5()
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child(format!("{} characters", metrics.characters))
                .child("·")
                .child(format!("{} lines", metrics.lines))
                .child("·")
                .child(format!("{} variables", metrics.variables)),
        )
        .into_any_element()
}
