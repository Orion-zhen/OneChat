use super::*;

impl OneChat {
    pub(crate) fn create_conversation(&mut self, cx: &mut Context<Self>) {
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
            self.navigation.page = Page::Settings;
            self.settings_ui.section = SettingsSection::DefaultModels;
            self.settings_ui.default_model_menu = Some(DefaultModelRole::Primary);
            self.data.error = Some("Choose a model before creating a conversation.".into());
            cx.notify();
            return;
        };
        if let Err(reason) = self.model_availability(&model) {
            self.navigation.page = Page::Settings;
            self.settings_ui.section = SettingsSection::DefaultModels;
            self.settings_ui.default_model_menu = Some(DefaultModelRole::Primary);
            self.data.error = Some(format!(
                "Choose an available model before creating a conversation: {reason}."
            ));
            cx.notify();
            return;
        }
        let conversation = Conversation::new(
            "New conversation",
            Some(&model),
            &self.data.snapshot.settings.default_system_prompt,
        );
        let id = conversation.id.clone();
        let mut settings = self.data.snapshot.settings.clone();
        settings.current_conversation_id = Some(id);
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        self.mutate_and_reload(
            move |storage| {
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
            self.navigation.page = Page::Chat;
            cx.notify();
            return;
        }
        let mut settings = self.data.snapshot.settings.clone();
        settings.current_conversation_id = Some(id);
        self.data.snapshot.settings = settings.clone();
        self.data.snapshot.current_turns.clear();
        self.data.snapshot.current_requests.clear();
        self.navigation.page = Page::Chat;
        self.reset_conversation_ui(cx);
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        self.mutate_and_reload(move |storage| storage.save_settings(&settings), cx);
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
        let event_id = conversation_id.clone();
        let input = cx.new(|cx| Composer::single_line(title, "Conversation title", cx));
        cx.subscribe(&input, move |this, _, event, cx| match event {
            ComposerEvent::Submit(title) => {
                this.finish_rename(&event_id, title.clone(), cx);
            }
            ComposerEvent::Cancel => {
                this.sidebar.rename_editor = None;
                cx.notify();
            }
            ComposerEvent::Changed(_) | ComposerEvent::Navigate(_) => {}
        })
        .detach();
        window.focus(&input.read(cx).focus_handle(cx));
        self.sidebar.rename_editor = Some(RenameEditor {
            conversation_id,
            input,
        });
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

    pub(crate) fn request_delete_conversation(&mut self, id: String, cx: &mut Context<Self>) {
        self.overlays.destructive_action = Some(DestructiveAction::DeleteConversation { id });
        self.navigation.pending_focus = Some(PendingFocus::Root);
        self.overlays.command_palette_open = false;
        self.overlays.model_picker_open = false;
        cx.notify();
    }

    fn delete_conversation(&mut self, id: String, cx: &mut Context<Self>) {
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
        self.overlays.destructive_action = None;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn confirm_destructive_action(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.overlays.destructive_action.take() else {
            return;
        };
        match action {
            DestructiveAction::DeleteConversation { id } => self.delete_conversation(id, cx),
            DestructiveAction::DeleteProvider { id } => self.delete_provider(id, cx),
            DestructiveAction::DeleteModel { id } => self.delete_model(id, cx),
            DestructiveAction::ClearContext { conversation_id } => {
                self.clear_current_context(conversation_id, cx)
            }
        }
    }

    pub(crate) fn dismiss_error(&mut self, cx: &mut Context<Self>) {
        self.data.error = None;
        cx.notify();
    }

    pub(crate) fn theme(&self) -> Theme {
        self.data.snapshot.settings.theme
    }

    pub(crate) fn settings(&self) -> &AppSettings {
        &self.data.snapshot.settings
    }
}
