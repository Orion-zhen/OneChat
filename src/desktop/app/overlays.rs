use super::*;

impl OneChat {
    pub(crate) fn conversation_groups(&self) -> Vec<(ConversationGroup, Vec<Conversation>)> {
        let query = self.sidebar.search_query.trim().to_lowercase();
        let now = now_timestamp();
        let mut groups = Vec::new();
        for group in [
            ConversationGroup::Pinned,
            ConversationGroup::Today,
            ConversationGroup::Yesterday,
            ConversationGroup::PreviousSevenDays,
            ConversationGroup::Older,
        ] {
            let conversations = self
                .data
                .snapshot
                .conversations
                .iter()
                .filter(|conversation| {
                    (query.is_empty() || conversation.title.to_lowercase().contains(&query))
                        && ConversationGroup::for_conversation(conversation, now) == group
                })
                .cloned()
                .collect::<Vec<_>>();
            if !conversations.is_empty() {
                groups.push((group, conversations));
            }
        }
        groups
    }

    pub(crate) fn rename_input(&self, conversation_id: &str) -> Option<Entity<Composer>> {
        self.sidebar
            .rename_editor
            .as_ref()
            .filter(|editor| editor.conversation_id == conversation_id)
            .map(|editor| editor.input.clone())
    }

    pub(crate) fn set_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.navigation.page = page;
        self.overlays.command_palette_open = false;
        self.overlays.model_picker_open = false;
        self.overlays.prompt_picker_open = false;
        self.overlays.response_model_turn_id = None;
        self.settings_ui.default_model_menu = None;
        cx.notify();
    }

    pub(crate) fn open_command_palette(&mut self, cx: &mut Context<Self>) {
        self.overlays.model_picker_open = false;
        self.overlays.prompt_picker_open = false;
        self.overlays.response_model_turn_id = None;
        self.overlays.command_palette_open = true;
        self.overlays.command_selection = 0;
        self.overlays
            .command_input
            .update(cx, |input, cx| input.set_text("", cx));
        self.navigation.pending_focus = Some(PendingFocus::CommandPalette);
        cx.notify();
    }

    pub(crate) fn close_command_palette(&mut self, cx: &mut Context<Self>) {
        self.overlays.command_palette_open = false;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn navigate_command(&mut self, direction: PickerDirection, cx: &mut Context<Self>) {
        self.overlays.command_selection = moved_selection(
            self.overlays.command_selection,
            self.filtered_commands().len(),
            direction,
        );
        self.overlays
            .command_scroll
            .scroll_to_item(self.overlays.command_selection);
        cx.notify();
    }

    pub(crate) fn confirm_command(&mut self, cx: &mut Context<Self>) {
        let commands = self.filtered_commands();
        let Some(command) = commands.get(self.overlays.command_selection).copied() else {
            return;
        };
        self.execute_command(command, cx);
    }

    pub(crate) fn execute_command(&mut self, command: PaletteCommand, cx: &mut Context<Self>) {
        self.overlays.command_palette_open = false;
        match command {
            PaletteCommand::NewConversation => {
                self.navigation.pending_focus = Some(PendingFocus::Composer);
                self.create_conversation(cx);
            }
            PaletteCommand::ChooseModel => self.open_model_picker(cx),
            PaletteCommand::FocusConversationSearch => {
                if self.data.snapshot.settings.sidebar_collapsed {
                    self.data.snapshot.settings.sidebar_collapsed = false;
                    self.save_settings(cx);
                }
                self.navigation.pending_focus = Some(PendingFocus::ConversationSearch);
                cx.notify();
            }
            PaletteCommand::ToggleSidebar => {
                self.navigation.pending_focus = Some(PendingFocus::Composer);
                self.toggle_sidebar(cx);
            }
            PaletteCommand::ToggleInspector => {
                self.navigation.pending_focus = Some(PendingFocus::Composer);
                self.toggle_inspector_immediate(cx);
            }
            PaletteCommand::EditSystemPrompt => {
                self.navigation.page = Page::Chat;
                if self.current_conversation().is_some() {
                    self.begin_edit_system_prompt(cx);
                } else {
                    self.data.error = Some("Create or select a conversation first.".into());
                    cx.notify();
                }
            }
            PaletteCommand::OpenChat => {
                self.navigation.page = Page::Chat;
                self.navigation.pending_focus = Some(PendingFocus::Composer);
                cx.notify();
            }
            PaletteCommand::OpenSettings => self.set_page(Page::Settings, cx),
        }
    }

    pub(crate) fn dismiss_overlay(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.chat.generation_config_editor
            && editor.parameter_menu_open
        {
            editor.close_menu();
            cx.notify();
        } else if self.settings_ui.default_model_menu.is_some() {
            self.settings_ui.default_model_menu = None;
            cx.notify();
        } else if self.settings_ui.default_prompt_menu_open {
            self.settings_ui.default_prompt_menu_open = false;
            cx.notify();
        } else if let Some(editor) = &mut self.settings_ui.provider_editor
            && editor.kind_menu_open
        {
            editor.kind_menu_open = false;
            cx.notify();
        } else if self.settings_ui.prompt_preset_editor.is_some() {
            self.cancel_prompt_preset_edit(cx);
        } else if self.settings_ui.viewed_prompt_preset.is_some() {
            self.close_prompt_preset_view(cx);
        } else if self.overlays.destructive_action.is_some() {
            self.cancel_destructive_action(cx);
        } else if self.overlays.command_palette_open {
            self.close_command_palette(cx);
        } else if self.overlays.model_picker_open {
            self.close_model_picker(cx);
        } else if self.overlays.prompt_picker_open {
            self.close_prompt_picker(cx);
        }
    }

    pub(crate) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.data.snapshot.settings.sidebar_collapsed =
            !self.data.snapshot.settings.sidebar_collapsed;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn toggle_inspector(&mut self, cx: &mut Context<Self>) {
        self.set_inspector_open(!self.navigation.inspector_open, true, cx);
    }

    pub(crate) fn toggle_inspector_immediate(&mut self, cx: &mut Context<Self>) {
        self.set_inspector_open(!self.navigation.inspector_open, false, cx);
    }

    pub(super) fn set_inspector_open(
        &mut self,
        open: bool,
        animated: bool,
        cx: &mut Context<Self>,
    ) {
        self.navigation.inspector_open = open;
        self.navigation.inspector_motion.set_open(open, animated);
        if open {
            self.sync_generation_config_editor(cx);
        }
        cx.notify();
    }

    pub(crate) fn set_inspector_tab(&mut self, tab: InspectorTab, cx: &mut Context<Self>) {
        self.navigation.inspector_tab = tab;
        if tab == InspectorTab::Model {
            self.sync_generation_config_editor(cx);
        }
        cx.notify();
    }

    pub(crate) fn open_model_picker(&mut self, cx: &mut Context<Self>) {
        self.overlays.command_palette_open = false;
        self.overlays.prompt_picker_open = false;
        self.overlays.response_model_turn_id = None;
        self.overlays.model_picker_open = true;
        self.overlays
            .model_search_input
            .update(cx, |input, cx| input.set_text("", cx));
        self.overlays.model_selection = self.initial_model_selection();
        self.navigation.pending_focus = Some(PendingFocus::ModelPicker);
        cx.notify();
    }

    pub(crate) fn open_response_model_picker(&mut self, turn_id: String, cx: &mut Context<Self>) {
        if self.is_current_generating() {
            return;
        }
        self.overlays.command_palette_open = false;
        self.overlays.prompt_picker_open = false;
        self.overlays.response_model_turn_id = Some(turn_id);
        self.overlays.model_picker_open = true;
        self.overlays
            .model_search_input
            .update(cx, |input, cx| input.set_text("", cx));
        self.overlays.model_selection = self.initial_model_selection();
        self.navigation.pending_focus = Some(PendingFocus::ModelPicker);
        cx.notify();
    }

    pub(crate) fn close_model_picker(&mut self, cx: &mut Context<Self>) {
        self.overlays.model_picker_open = false;
        self.overlays.response_model_turn_id = None;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn open_prompt_picker(&mut self, cx: &mut Context<Self>) {
        if self.current_conversation().is_none() || self.is_current_generating() {
            return;
        }
        self.overlays.command_palette_open = false;
        self.overlays.model_picker_open = false;
        self.overlays.response_model_turn_id = None;
        self.overlays.prompt_picker_open = true;
        self.reload_snapshot(cx);
        cx.notify();
    }

    pub(crate) fn close_prompt_picker(&mut self, cx: &mut Context<Self>) {
        self.overlays.prompt_picker_open = false;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn select_system_prompt_preset(
        &mut self,
        name: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.is_current_generating() {
            return;
        }
        let Some(mut conversation) = self.current_conversation().cloned() else {
            return;
        };
        self.overlays.prompt_picker_open = false;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        self.mutate_and_reload(
            move |storage| {
                let content = match name {
                    Some(name) => storage
                        .load_prompt_preset(&name)?
                        .map(|preset| preset.content)
                        .ok_or_else(|| {
                            StorageError::InvalidData(format!("prompt preset not found: {name}"))
                        })?,
                    None => String::new(),
                };
                if conversation.system_prompt != content {
                    conversation.system_prompt = content;
                    conversation.updated_at = now_timestamp();
                    storage.update_conversation(&conversation)?;
                }
                Ok(())
            },
            cx,
        );
    }

    pub(crate) fn open_prompt_settings(&mut self, cx: &mut Context<Self>) {
        self.overlays.prompt_picker_open = false;
        self.navigation.page = Page::Settings;
        self.settings_ui.section = SettingsSection::SystemPrompts;
        self.reload_snapshot(cx);
        cx.notify();
    }

    pub(crate) fn navigate_model(&mut self, direction: PickerDirection, cx: &mut Context<Self>) {
        let models = self.filtered_models();
        let mut selection = self.overlays.model_selection;
        for _ in 0..models.len() {
            selection = moved_selection(selection, models.len(), direction);
            if self.model_availability(models[selection]).is_ok() {
                break;
            }
        }
        self.overlays.model_selection = selection;
        self.overlays.model_scroll.scroll_to_item(selection);
        cx.notify();
    }

    pub(crate) fn confirm_model(&mut self, cx: &mut Context<Self>) {
        let model_id = self
            .filtered_models()
            .get(self.overlays.model_selection)
            .filter(|model| self.model_availability(model).is_ok())
            .map(|model| model.id.clone());
        if let Some(model_id) = model_id {
            self.select_model(model_id, cx);
        }
    }

    pub(crate) fn select_model(&mut self, model_id: String, cx: &mut Context<Self>) {
        let Some(model) = self
            .data
            .snapshot
            .models
            .iter()
            .find(|model| model.id == model_id)
            .cloned()
        else {
            return;
        };
        if let Err(reason) = self.model_availability(&model) {
            self.data.error = Some(format!("Model is unavailable: {reason}."));
            cx.notify();
            return;
        }
        let conversation = self.current_conversation().cloned();
        let response_turn_id = self.overlays.response_model_turn_id.take();
        self.overlays.model_picker_open = false;
        self.navigation.pending_focus = Some(if conversation.is_some() {
            PendingFocus::Composer
        } else {
            PendingFocus::Root
        });

        if let Some(turn_id) = response_turn_id {
            self.start_additional_response(turn_id, model.id, cx);
            return;
        }

        let Some(mut conversation) = conversation else {
            self.chat.draft_model_id = Some(model.id);
            cx.notify();
            return;
        };
        if conversation.model_id.as_deref() == Some(&model.id) {
            cx.notify();
            return;
        }
        conversation.model_id = Some(model.id);
        conversation.updated_at = now_timestamp();
        self.chat.parameter_error = None;
        self.mutate_and_reload(
            move |storage| storage.update_conversation(&conversation),
            cx,
        );
    }
}
