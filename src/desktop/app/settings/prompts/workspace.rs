use super::*;
use gpui::{prelude::*, px};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    dialog::DialogFooter,
    input::{InputEvent, InputState},
};

use crate::desktop::{
    app::SettingsDestination,
    ui::{
        icons::{AppIcon, IconTone, render_icon},
        settings::{
            PromptPresetSection, PromptPresetWorkspace, PromptPresetWorkspaceMode, SettingsSection,
        },
    },
};

impl OneChat {
    pub(in crate::desktop::app::settings::prompts) fn install_prompt_preset_workspace(
        &mut self,
        workspace: PromptPresetWorkspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = workspace.editor.name.clone();
        let textareas = [
            workspace.editor.system_prompt.clone(),
            workspace.editor.assistant_opening.clone(),
        ];
        self.settings_ui.prompt_preset_workspace = Some(workspace);
        self.settings_ui.pending_prompt_preset_exit = None;
        self.settings_ui.form_error = None;

        self.subscribe_prompt_preset_name(name, window, cx);
        for textarea in textareas {
            cx.subscribe_in(&textarea, window, |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.settings_ui.form_error = None;
                    cx.notify();
                }
            })
            .detach();
        }
        cx.notify();
    }

    fn subscribe_prompt_preset_name(
        &mut self,
        input: gpui::Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.subscribe_in(&input, window, |this, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.settings_ui.form_error = None;
                cx.notify();
            }
        })
        .detach();
    }

    pub(crate) fn edit_viewed_prompt_preset(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = &mut self.settings_ui.prompt_preset_workspace else {
            return;
        };
        workspace.mode = PromptPresetWorkspaceMode::Edit;
        let input = workspace.editor.input(workspace.section);
        input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    pub(crate) fn select_prompt_preset_section(
        &mut self,
        section: PromptPresetSection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = &mut self.settings_ui.prompt_preset_workspace else {
            return;
        };
        workspace.section = section;
        if workspace.is_editing() {
            let input = workspace.editor.input(section);
            input.update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }

    pub(crate) fn toggle_prompt_preset_inspector(&mut self, cx: &mut Context<Self>) {
        if let Some(workspace) = &mut self.settings_ui.prompt_preset_workspace {
            workspace.inspector_open = !workspace.inspector_open;
            cx.notify();
        }
    }

    pub(crate) fn toggle_prompt_preset_focus_mode(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = &mut self.settings_ui.prompt_preset_workspace else {
            return;
        };
        workspace.focus_mode = !workspace.focus_mode;
        if workspace.is_editing() {
            let input = workspace.editor.input(workspace.section);
            input.update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }

    pub(crate) fn insert_prompt_preset_variable(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = &self.settings_ui.prompt_preset_workspace else {
            return;
        };
        if !workspace.is_editing() {
            return;
        }
        let input = workspace.editor.input(workspace.section);
        input.update(cx, |input, cx| {
            input.insert(format!("{{{{{name}}}}}"), window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    pub(crate) fn duplicate_prompt_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = &self.settings_ui.prompt_preset_workspace else {
            return;
        };
        let current_name = workspace.editor.name.read(cx).value().trim().to_string();
        let base = if current_name.is_empty() {
            "Untitled Preset".to_string()
        } else {
            format!("{current_name} Copy")
        };
        let mut duplicate_name = base.clone();
        let mut suffix = 2;
        while self.prompt_preset(&duplicate_name).is_some() {
            duplicate_name = format!("{base} {suffix}");
            suffix += 1;
        }

        let workspace = self
            .settings_ui
            .prompt_preset_workspace
            .as_mut()
            .expect("workspace checked above");
        workspace.editor.make_duplicate(duplicate_name, window, cx);
        workspace.mode = PromptPresetWorkspaceMode::Edit;
        workspace.inspector_open = true;
        workspace.focus_mode = false;
        workspace
            .editor
            .name
            .update(cx, |input, cx| input.focus(window, cx));
        self.settings_ui.form_error = None;
        cx.notify();
    }

    pub(crate) fn request_close_prompt_preset_workspace(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_prompt_preset_exit(
            SettingsDestination::Section(SettingsSection::SystemPrompts),
            window,
            cx,
        );
    }

    pub(in crate::desktop::app) fn request_prompt_preset_exit(
        &mut self,
        destination: SettingsDestination,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = &self.settings_ui.prompt_preset_workspace else {
            self.apply_settings_destination(destination, window, cx);
            return;
        };
        if !workspace.is_dirty(cx) {
            self.apply_settings_destination(destination, window, cx);
            return;
        }

        self.settings_ui.pending_prompt_preset_exit = Some(destination);
        let app = cx.entity();
        let cancel_app = app.clone();
        let save_app = app.clone();
        let cancel_click_app = app.clone();
        let discard_click_app = app.clone();
        let save_click_app = app.clone();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let cancel_app = cancel_app.clone();
            let save_app = save_app.clone();
            let cancel_click_app = cancel_click_app.clone();
            let discard_click_app = discard_click_app.clone();
            let save_click_app = save_click_app.clone();
            dialog
                .title("Save prompt preset changes?")
                .description("Your changes will be lost if you leave without saving.")
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("cancel-prompt-preset-exit")
                                .ghost()
                                .tooltip("Keep editing")
                                .size(px(36.0))
                                .p_0()
                                .child(render_icon(AppIcon::Close, IconTone::Muted, 19.0, cx))
                                .on_click(move |_, window, cx| {
                                    cancel_click_app
                                        .update(cx, |app, cx| app.cancel_prompt_preset_exit(cx));
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            Button::new("discard-prompt-preset-changes")
                                .danger()
                                .tooltip("Discard changes")
                                .size(px(36.0))
                                .p_0()
                                .child(render_icon(AppIcon::Trash, IconTone::OnAccent, 19.0, cx))
                                .on_click(move |_, window, cx| {
                                    discard_click_app.update(cx, |app, cx| {
                                        app.discard_prompt_preset_changes(window, cx)
                                    });
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            Button::new("save-prompt-preset-before-exit")
                                .primary()
                                .tooltip("Save changes")
                                .size(px(36.0))
                                .p_0()
                                .child(render_icon(AppIcon::Save, IconTone::OnAccent, 19.0, cx))
                                .on_click(move |_, window, cx| {
                                    let saved = save_click_app.update(cx, |app, cx| {
                                        app.save_prompt_preset_before_exit(window, cx)
                                    });
                                    if saved {
                                        window.close_dialog(cx);
                                    }
                                }),
                        ),
                )
                .on_ok(move |_, window, cx| {
                    save_app.update(cx, |app, cx| app.save_prompt_preset_before_exit(window, cx))
                })
                .on_cancel(move |_, _, cx| {
                    cancel_app.update(cx, |app, cx| app.cancel_prompt_preset_exit(cx));
                    true
                })
        });
    }

    fn cancel_prompt_preset_exit(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.pending_prompt_preset_exit = None;
        cx.notify();
    }

    fn discard_prompt_preset_changes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(destination) = self.settings_ui.pending_prompt_preset_exit.take() {
            self.apply_settings_destination(destination, window, cx);
        }
    }

    fn save_prompt_preset_before_exit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(destination) = self.settings_ui.pending_prompt_preset_exit.clone() else {
            return false;
        };
        if !self.save_prompt_preset(cx) {
            return false;
        }
        self.apply_settings_destination(destination, window, cx);
        true
    }
}
