mod appearance;
mod catalog;
mod mcp;
mod prompts;

use gpui::{Context, Window, prelude::*, px};
use gpui_component::{
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::DialogFooter,
};

use super::{OneChat, Page, SettingsDestination};
use crate::desktop::ui::{
    icons::{AppIcon, IconTone, render_icon},
    settings::SettingsSection,
};

impl OneChat {
    pub(crate) fn request_select_settings_section(
        &mut self,
        section: SettingsSection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_ui.section == section && self.settings_ui.prompt_preset_workspace.is_none()
        {
            return;
        }
        self.request_settings_destination(SettingsDestination::Section(section), window, cx);
    }

    pub(crate) fn request_add_provider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_ui.section == SettingsSection::NewProvider {
            return;
        }
        self.request_settings_destination(SettingsDestination::AddProvider, window, cx);
    }

    pub(crate) fn request_leave_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request_settings_destination(SettingsDestination::Page(Page::Chat), window, cx);
    }

    fn request_settings_destination(
        &mut self,
        destination: SettingsDestination,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_ui.prompt_preset_workspace.is_some() {
            self.request_prompt_preset_exit(destination, window, cx);
        } else {
            self.request_provider_editor_exit(destination, window, cx);
        }
    }

    pub(in crate::desktop::app) fn request_provider_editor_exit(
        &mut self,
        destination: SettingsDestination,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = &self.settings_ui.provider_editor else {
            self.apply_settings_destination(destination, window, cx);
            return;
        };
        if editor.saving {
            return;
        }
        if !editor.is_dirty(cx) {
            self.apply_settings_destination(destination, window, cx);
            return;
        }

        self.settings_ui.pending_provider_exit = Some(destination);
        let app = cx.entity();
        let cancel_app = app.clone();
        let discard_app = app.clone();
        let cancel_click_app = app.clone();
        let discard_click_app = app.clone();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let cancel_app = cancel_app.clone();
            let discard_app = discard_app.clone();
            let cancel_click_app = cancel_click_app.clone();
            let discard_click_app = discard_click_app.clone();
            dialog
                .title("Discard provider changes?")
                .description("Your unsaved provider changes will be lost.")
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("cancel-provider-exit")
                                .ghost()
                                .tooltip("Keep editing")
                                .size(px(36.0))
                                .p_0()
                                .child(render_icon(AppIcon::Close, IconTone::Muted, 19.0, cx))
                                .on_click(move |_, window, cx| {
                                    cancel_click_app
                                        .update(cx, |app, cx| app.cancel_provider_editor_exit(cx));
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            Button::new("discard-provider-changes")
                                .danger()
                                .tooltip("Discard changes")
                                .size(px(36.0))
                                .p_0()
                                .child(render_icon(AppIcon::Trash, IconTone::OnAccent, 19.0, cx))
                                .on_click(move |_, window, cx| {
                                    discard_click_app.update(cx, |app, cx| {
                                        app.confirm_provider_editor_exit(window, cx)
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
                .on_ok(move |_, window, cx| {
                    discard_app.update(cx, |app, cx| app.confirm_provider_editor_exit(window, cx));
                    true
                })
                .on_cancel(move |_, _, cx| {
                    cancel_app.update(cx, |app, cx| app.cancel_provider_editor_exit(cx));
                    true
                })
        });
    }

    fn cancel_provider_editor_exit(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.pending_provider_exit = None;
        cx.notify();
    }

    fn confirm_provider_editor_exit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(destination) = self.settings_ui.pending_provider_exit.take() {
            self.apply_settings_destination(destination, window, cx);
        }
    }

    pub(in crate::desktop::app) fn apply_settings_destination(
        &mut self,
        destination: SettingsDestination,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_ui.pending_provider_exit = None;
        self.settings_ui.pending_prompt_preset_exit = None;
        self.settings_ui.provider_editor = None;
        self.settings_ui.prompt_preset_workspace = None;
        self.settings_ui.form_error = None;
        match destination {
            SettingsDestination::Section(section) => self.select_settings_section(section, cx),
            SettingsDestination::Page(page) => self.set_page(page, cx),
            SettingsDestination::AddProvider => self.begin_add_provider(window, cx),
        }
        cx.notify();
    }

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
        self.settings_ui.prompt_preset_workspace = None;
        self.settings_ui.pending_prompt_preset_exit = None;
        self.settings_ui.provider_editor = None;
        self.settings_ui.pending_provider_exit = None;
        self.settings_ui.model_editor = None;
        self.settings_ui.title_prompt_editor = None;
        self.settings_ui.translation_system_prompt_editor = None;
        self.settings_ui.translation_user_prompt_editor = None;
        self.settings_ui.mcp_server_editor = None;
        self.settings_ui.mcp_error = None;
        self.settings_ui.form_error = None;
        if reload_prompts {
            self.reload_snapshot(cx);
        }
        cx.notify();
    }
}
