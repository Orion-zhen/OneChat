use gpui::{Context, Focusable as _, Window, prelude::*, px};
use gpui_component::{
    WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::DialogFooter,
    input::{InputEvent, InputState},
};

use super::{DestructiveAction, OneChat, Page, PendingFocus, RenameEditor};
use crate::{
    desktop::ui::settings::SettingsSection,
    domain::{AppSettings, Conversation, HistoryLimit, Theme, now_timestamp},
};

fn resolve_destructive_action(
    action: &mut Option<DestructiveAction>,
    confirmed: bool,
) -> Option<DestructiveAction> {
    let action = action.take();
    if confirmed { action } else { None }
}

impl OneChat {
    pub(crate) fn create_conversation(&mut self, cx: &mut Context<Self>) {
        self.cancel_voice_recording(cx);
        let model_id = if self.current_conversation().is_none() {
            self.chat.draft_model_id.as_deref()
        } else {
            None
        }
        .or(self.data.snapshot.settings.primary_model_id.as_deref());
        let model = model_id
            .and_then(|id| {
                self.data
                    .snapshot
                    .models
                    .iter()
                    .find(|model| model.id == id)
            })
            .cloned();
        let Some(model) = model else {
            self.set_page(Page::Settings, cx);
            self.settings_ui.section = SettingsSection::DefaultModels;
            self.data.error = Some("Choose a model before creating a conversation.".into());
            cx.notify();
            return;
        };
        if let Err(reason) = self.model_availability(&model) {
            self.set_page(Page::Settings, cx);
            self.settings_ui.section = SettingsSection::DefaultModels;
            self.data.error = Some(format!(
                "Choose an available model before creating a conversation: {reason}."
            ));
            cx.notify();
            return;
        }
        let conversation = Conversation::new("New conversation", Some(&model), "");
        let id = conversation.id.clone();
        let mut settings = self.data.snapshot.settings.clone();
        settings.current_conversation_id = Some(id);
        let default_prompt_name = settings
            .default_system_prompt_preset
            .clone()
            .filter(|name| self.prompt_preset(name).is_some());
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        self.mutate_and_reload(
            move |storage| {
                let mut conversation = conversation;
                conversation.system_prompt = default_prompt_name
                    .as_deref()
                    .map(|name| storage.load_prompt_preset(name))
                    .transpose()?
                    .flatten()
                    .map(|preset| preset.content)
                    .unwrap_or_default();
                storage.insert_conversation(&conversation)?;
                storage.save_settings(&settings)
            },
            cx,
        );
    }

    pub(crate) fn select_conversation(&mut self, id: String, cx: &mut Context<Self>) {
        if self
            .data
            .snapshot
            .settings
            .current_conversation_id
            .as_deref()
            == Some(&id)
        {
            self.set_page(Page::Chat, cx);
            return;
        }
        let mut settings = self.data.snapshot.settings.clone();
        settings.current_conversation_id = Some(id);
        self.data.snapshot.settings = settings.clone();
        self.data.snapshot.current_turns.clear();
        self.data.snapshot.current_requests.clear();
        self.set_page(Page::Chat, cx);
        self.reset_conversation_ui(cx);
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        self.mutate_and_reload(move |storage| storage.save_settings(&settings), cx);
    }

    pub(crate) fn preview_conversation_history_limit(
        &mut self,
        value: f32,
        cx: &mut Context<Self>,
    ) {
        if self.is_current_generating() {
            return;
        }
        let Some(conversation) = self.current_conversation() else {
            return;
        };
        let limit = HistoryLimit::from_slider_value(value);
        let effective = self.effective_history_limit(conversation);
        let preview = (limit != effective).then_some(limit);
        if self.chat.history_limit_preview == preview {
            return;
        }
        self.chat.history_limit_preview = preview;
        cx.notify();
    }

    pub(crate) fn commit_conversation_history_limit(&mut self, value: f32, cx: &mut Context<Self>) {
        if self.is_current_generating() {
            self.chat.history_limit_preview = None;
            cx.notify();
            return;
        }
        self.chat.history_limit_preview = None;
        let global = self.settings().history_limit;
        let limit = HistoryLimit::from_slider_value(value);
        let Some(mut conversation) = self.current_conversation().cloned() else {
            return;
        };
        let original = conversation.history_limit_override;
        let history_limit_override = if original.is_none() && limit == global {
            None
        } else {
            Some(limit)
        };
        if history_limit_override == original {
            cx.notify();
            return;
        }
        conversation.history_limit_override = history_limit_override;
        conversation.updated_at = now_timestamp();
        if let Some(stored) = self
            .data
            .snapshot
            .conversations
            .iter_mut()
            .find(|stored| stored.id == conversation.id)
        {
            *stored = conversation.clone();
        }
        cx.notify();
        self.mutate_and_reload(
            move |storage| storage.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn reset_conversation_history_limit(&mut self, cx: &mut Context<Self>) {
        if self.is_current_generating() {
            return;
        }
        self.chat.history_limit_preview = None;
        let Some(mut conversation) = self
            .current_conversation()
            .filter(|conversation| conversation.history_limit_override.is_some())
            .cloned()
        else {
            return;
        };
        conversation.history_limit_override = None;
        conversation.updated_at = now_timestamp();
        if let Some(stored) = self
            .data
            .snapshot
            .conversations
            .iter_mut()
            .find(|stored| stored.id == conversation.id)
        {
            *stored = conversation.clone();
        }
        cx.notify();
        self.mutate_and_reload(
            move |storage| storage.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn start_rename(
        &mut self,
        conversation_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(conversation) = self
            .data
            .snapshot
            .conversations
            .iter()
            .find(|conversation| conversation.id == conversation_id)
        else {
            return;
        };
        let title = conversation.title.clone();
        let cursor = title.len();
        let event_id = conversation_id.clone();
        let input = cx.new(|cx| {
            let mut input = InputState::new(window, cx)
                .default_value(title)
                .placeholder("Conversation title")
                .submit_on_enter(true);
            input.set_selected_range(cursor..cursor, cx);
            input
        });
        cx.subscribe_in(
            &input,
            window,
            move |this, input, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::PressEnter { shift: false, .. }) {
                    this.finish_rename(&event_id, input.read(cx).value().to_string(), cx);
                }
            },
        )
        .detach();
        window.focus(&input.read(cx).focus_handle(cx), cx);
        self.sidebar.rename_editor = Some(RenameEditor {
            conversation_id,
            input,
        });
        cx.notify();
    }

    pub(crate) fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.sidebar.rename_editor = None;
        cx.notify();
    }

    fn finish_rename(&mut self, id: &str, title: String, cx: &mut Context<Self>) {
        let title = title.trim();
        if title.is_empty() {
            return;
        }
        if !self
            .data
            .snapshot
            .conversations
            .iter()
            .any(|conversation| conversation.id == id)
        {
            return;
        }
        let id = id.to_string();
        let title = title.to_string();
        self.sidebar.rename_editor = None;
        self.mutate_and_reload(move |storage| storage.rename_conversation(&id, &title), cx);
    }

    pub(crate) fn toggle_pin(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(mut conversation) = self
            .data
            .snapshot
            .conversations
            .iter()
            .find(|conversation| conversation.id == id)
            .cloned()
        else {
            return;
        };
        conversation.pinned = !conversation.pinned;
        conversation.updated_at = now_timestamp();
        self.mutate_and_reload(
            move |storage| storage.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn request_delete_conversation(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_destructive_action(DestructiveAction::DeleteConversation { id }, window, cx);
    }

    pub(super) fn request_destructive_action(
        &mut self,
        action: DestructiveAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.chat.text_selection.clear(window);
        let title = action.title();
        let description = action.description();
        self.overlays.destructive_action = Some(action);

        let confirm_app = cx.entity();
        let cancel_app = confirm_app.clone();
        let confirm_click_app = confirm_app.clone();
        let cancel_click_app = confirm_app.clone();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let confirm_app = confirm_app.clone();
            let cancel_app = cancel_app.clone();
            let confirm_click_app = confirm_click_app.clone();
            let cancel_click_app = cancel_click_app.clone();
            dialog
                .title(title)
                .description(description)
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("cancel-destructive-action")
                                .ghost()
                                .tooltip("Cancel")
                                .size(px(36.0))
                                .p_0()
                                .child(crate::desktop::ui::icons::render_icon(
                                    crate::desktop::ui::icons::AppIcon::Close,
                                    crate::desktop::ui::icons::IconTone::Muted,
                                    19.0,
                                    cx,
                                ))
                                .on_click(move |_, window, cx| {
                                    cancel_click_app
                                        .update(cx, |app, cx| app.cancel_destructive_action(cx));
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            Button::new("confirm-destructive-action")
                                .danger()
                                .tooltip("Confirm destructive action")
                                .size(px(36.0))
                                .p_0()
                                .child(crate::desktop::ui::icons::render_icon(
                                    crate::desktop::ui::icons::AppIcon::Trash,
                                    crate::desktop::ui::icons::IconTone::OnAccent,
                                    19.0,
                                    cx,
                                ))
                                .on_click(move |_, window, cx| {
                                    confirm_click_app
                                        .update(cx, |app, cx| app.confirm_destructive_action(cx));
                                    window.close_dialog(cx);
                                }),
                        ),
                )
                .on_ok(move |_, _, cx| {
                    confirm_app.update(cx, |app, cx| app.confirm_destructive_action(cx));
                    true
                })
                .on_cancel(move |_, _, cx| {
                    cancel_app.update(cx, |app, cx| app.cancel_destructive_action(cx));
                    true
                })
        });
    }

    pub(crate) fn delete_conversation(&mut self, id: String, cx: &mut Context<Self>) {
        self.chat.generations.stop(&id);
        let mut settings = self.data.snapshot.settings.clone();
        if settings.current_conversation_id.as_deref() == Some(&id) {
            settings.current_conversation_id = self
                .data
                .snapshot
                .conversations
                .iter()
                .find(|conversation| conversation.id != id)
                .map(|conversation| conversation.id.clone());
        }
        self.mutate_and_reload(
            move |storage| {
                storage.delete_conversation(&id)?;
                storage.save_settings(&settings)
            },
            cx,
        );
    }

    pub(crate) fn cancel_destructive_action(&mut self, cx: &mut Context<Self>) {
        resolve_destructive_action(&mut self.overlays.destructive_action, false);
        cx.notify();
    }

    pub(crate) fn confirm_destructive_action(&mut self, cx: &mut Context<Self>) {
        let Some(action) = resolve_destructive_action(&mut self.overlays.destructive_action, true)
        else {
            return;
        };
        match action {
            DestructiveAction::DeleteConversation { id } => self.delete_conversation(id, cx),
            DestructiveAction::DeleteProvider { id } => self.delete_provider(id, cx),
            DestructiveAction::DeleteModel { id } => self.delete_model(id, cx),
            DestructiveAction::DeletePromptPreset { name } => self.delete_prompt_preset(name, cx),
            DestructiveAction::DeletePromptVariable { name } => {
                self.delete_prompt_variable(name, cx)
            }
            DestructiveAction::DeleteMcpServer { id } => self.delete_mcp_server(id, cx),
            DestructiveAction::ClearContext { conversation_id } => {
                self.clear_current_context(conversation_id, cx)
            }
        }
    }

    pub(crate) fn theme(&self) -> Theme {
        self.data.snapshot.settings.theme
    }

    pub(crate) fn settings(&self) -> &AppSettings {
        &self.data.snapshot.settings
    }
}
