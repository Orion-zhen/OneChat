use gpui::{Context, Entity, Window};
use gpui_component::{WindowExt as _, input::InputState};

use super::{ConversationGroup, OneChat, Page, PaletteCommand, PendingFocus, ShellOverlay};
use crate::{
    desktop::ui::{
        SIDEBAR_WIDTH,
        inspector::InspectorTab,
        settings::SettingsSection,
        shell::{
            CommandPaletteDelegate, ConversationSearchDelegate, ConversationSearchResult,
            ModelPickerDelegate, PromptPickerDelegate, ReasoningPickerDelegate,
        },
    },
    domain::{Conversation, now_timestamp},
};

impl OneChat {
    pub(crate) fn conversation_groups(&self) -> Vec<(ConversationGroup, Vec<Conversation>)> {
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
                    !conversation.temporary
                        && !self.is_transient_conversation(&conversation.id)
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

    pub(crate) fn rename_input(&self, conversation_id: &str) -> Option<Entity<InputState>> {
        self.sidebar
            .rename_editor
            .as_ref()
            .filter(|editor| editor.conversation_id == conversation_id)
            .map(|editor| editor.input.clone())
    }

    pub(crate) fn set_page(&mut self, page: Page, cx: &mut Context<Self>) {
        #[cfg(target_os = "macos")]
        self.close_conversation_peek(cx);
        if page != Page::Chat {
            self.cancel_voice_recording(cx);
        }
        if page != Page::Tts {
            self.tts.view.connection_popover_open = false;
            self.tts.view.inspector_open = false;
            self.tts.inspector_motion.set_open(false, false);
        }
        if self.navigation.page != page {
            let sidebar_width = match page {
                Page::Chat | Page::Translate | Page::Tts if self.settings().sidebar_collapsed => {
                    0.0
                }
                Page::Chat | Page::Translate | Page::Tts => self.sidebar.width,
                Page::Settings => SIDEBAR_WIDTH,
            };
            self.navigation
                .sidebar_width_motion
                .set_target(sidebar_width, true);
            self.navigation.page = page;
        }
        self.overlays.response_model_turn_id = None;
        cx.notify();
    }

    pub(crate) fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.chat.text_selection.clear(window);
        self.overlays.response_model_turn_id = None;
        self.overlays.command_picker.update(cx, |picker, cx| {
            *picker.delegate_mut() = CommandPaletteDelegate::new();
            picker.set_query("", window, cx);
            picker.set_selected_index(Some(gpui_component::IndexPath::default()), window, cx);
        });
        self.open_shell_overlay(ShellOverlay::CommandPalette, true, window, cx);
        self.overlays
            .command_picker
            .update(cx, |picker, cx| picker.focus(window, cx));
    }

    pub(crate) fn execute_command(
        &mut self,
        command: PaletteCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match command {
            PaletteCommand::NewConversation => self.create_conversation(cx),
            PaletteCommand::ChooseModel => self.open_model_picker_immediate(window, cx),
            PaletteCommand::FocusConversationSearch => {
                self.open_conversation_search(window, cx);
            }
            PaletteCommand::ToggleSidebar => self.toggle_sidebar(cx),
            PaletteCommand::ToggleInspector => self.toggle_inspector_immediate(cx),
            PaletteCommand::EditSystemPrompt => {
                self.set_page(Page::Chat, cx);
                if self.current_conversation().is_some() {
                    self.begin_edit_system_prompt(window, cx);
                } else {
                    self.data.error = Some("Create or select a conversation first.".into());
                    cx.notify();
                }
            }
            PaletteCommand::OpenChat => {
                self.set_page(Page::Chat, cx);
                self.navigation.pending_focus = Some(PendingFocus::Composer);
                cx.notify();
            }
            PaletteCommand::OpenTranslation => self.set_page(Page::Translate, cx),
            PaletteCommand::OpenTextToSpeech => self.set_page(Page::Tts, cx),
            PaletteCommand::OpenSettings => self.set_page(Page::Settings, cx),
        }
    }

    pub(crate) fn dismiss_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) {
            self.overlays.response_model_turn_id = None;
            self.overlays.destructive_action = None;
            self.settings_ui.pending_provider_exit = None;
            self.settings_ui.pending_prompt_preset_exit = None;
            self.settings_ui.form_error = None;
            window.close_dialog(cx);
        } else if self.tts.view.connection_popover_open {
            self.tts.view.connection_popover_open = false;
            cx.notify();
        } else if self.tts.view.inspector_open {
            self.set_tts_inspector_open(false, cx);
        } else if self.overlays.active.is_some() {
            self.close_shell_overlay(false, cx);
        } else if self.settings_ui.prompt_preset_workspace.is_some() {
            self.request_close_prompt_preset_workspace(window, cx);
        } else if self.settings_ui.provider_editor.is_some() {
            self.cancel_provider_editor(window, cx);
        } else if self.settings_ui.title_prompt_editor.is_some() {
            self.cancel_title_prompt_edit(cx);
        } else if self.settings_ui.translation_system_prompt_editor.is_some() {
            self.cancel_translation_prompt_edit(cx);
        }
    }

    pub(crate) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        #[cfg(target_os = "macos")]
        self.close_conversation_peek(cx);
        self.data.snapshot.settings.sidebar_collapsed =
            !self.data.snapshot.settings.sidebar_collapsed;
        if matches!(
            self.navigation.page,
            Page::Chat | Page::Translate | Page::Tts
        ) {
            let width = if self.data.snapshot.settings.sidebar_collapsed {
                0.0
            } else {
                self.sidebar.width
            };
            self.navigation.sidebar_width_motion.set_target(width, true);
        }
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn set_inspector_visible(&mut self, open: bool, cx: &mut Context<Self>) {
        self.set_inspector_open(open, true, cx);
    }

    pub(crate) fn toggle_inspector_immediate(&mut self, cx: &mut Context<Self>) {
        self.set_inspector_open(!self.navigation.inspector_open, false, cx);
    }

    pub(crate) fn close_inspector(&mut self, cx: &mut Context<Self>) {
        self.set_inspector_visible(false, cx);
    }

    pub(super) fn set_inspector_open(
        &mut self,
        open: bool,
        animated: bool,
        cx: &mut Context<Self>,
    ) {
        self.navigation.inspector_open = open;
        self.navigation.inspector_motion.set_open(open, animated);
        cx.notify();
    }

    pub(crate) fn set_inspector_tab(&mut self, tab: InspectorTab, cx: &mut Context<Self>) {
        self.navigation.inspector_tab = tab;
        cx.notify();
    }

    pub(crate) fn open_tools_inspector(&mut self, cx: &mut Context<Self>) {
        self.navigation.inspector_tab = InspectorTab::Tools;
        self.set_inspector_visible(true, cx);
    }

    pub(crate) fn open_conversation_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.chat.text_selection.clear(window);
        let delegate = ConversationSearchDelegate::from_app(self);
        let selected = (delegate.row_count() > 0).then(gpui_component::IndexPath::default);
        self.overlays.conversation_search.update(cx, |search, cx| {
            *search.delegate_mut() = delegate;
            search.set_query("", window, cx);
            search.set_selected_index(selected, window, cx);
        });
        self.open_shell_overlay(ShellOverlay::ConversationSearch, true, window, cx);
        self.overlays
            .conversation_search
            .update(cx, |search, cx| search.focus(window, cx));
    }

    pub(crate) fn open_conversation_search_result(
        &mut self,
        result: ConversationSearchResult,
        cx: &mut Context<Self>,
    ) {
        self.close_shell_overlay(true, cx);
        self.overlays.previous_focus = None;
        let Some(target) = result.target else {
            self.select_conversation(result.conversation_id, cx);
            return;
        };

        let conversation_changed =
            self.current_conversation_id() != Some(result.conversation_id.as_str());
        if conversation_changed {
            if let Some(transient_id) = self.chat.transient_conversation_id.take() {
                self.chat.generations.stop(&transient_id);
                self.data
                    .snapshot
                    .conversations
                    .retain(|conversation| conversation.id != transient_id);
            }
            self.data.snapshot.current_turns.clear();
            self.data.snapshot.current_requests.clear();
            self.reset_conversation_ui(cx);
        } else {
            self.chat.visible_response_ids.clear();
        }

        let mut settings = self.data.snapshot.settings.clone();
        settings.current_conversation_id = Some(result.conversation_id.clone());
        self.data.snapshot.settings = settings.clone();
        self.chat.pending_search_target = Some(target.clone());
        self.set_page(Page::Chat, cx);
        let conversation_id = result.conversation_id;
        let turn_id = target.turn_id;
        self.mutate_and_reload(
            move |storage| {
                storage.select_turn_path(&conversation_id, &turn_id)?;
                storage.save_settings(&settings)
            },
            cx,
        );
    }

    pub(crate) fn open_model_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_model_picker_for_turn(None, true, window, cx);
    }

    pub(crate) fn open_model_picker_immediate(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_model_picker_for_turn(None, false, window, cx);
    }

    pub(crate) fn open_response_model_picker(
        &mut self,
        turn_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_current_generating() {
            return;
        }
        self.open_model_picker_for_turn(Some(turn_id), true, window, cx);
    }

    fn open_model_picker_for_turn(
        &mut self,
        turn_id: Option<String>,
        animated: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.chat.text_selection.clear(window);
        self.overlays.response_model_turn_id = turn_id;
        let delegate = ModelPickerDelegate::from_app(self);
        let selected = delegate.initial_selection();
        self.overlays.model_picker.update(cx, |picker, cx| {
            *picker.delegate_mut() = delegate;
            picker.set_query("", window, cx);
            picker.set_selected_index(selected, window, cx);
            picker.scroll_to_selected_item(window, cx);
        });

        self.open_shell_overlay(ShellOverlay::ModelPicker, animated, window, cx);
        self.overlays
            .model_picker
            .update(cx, |picker, cx| picker.focus(window, cx));
    }

    pub(crate) fn open_reasoning_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (model, unavailable) = if self.navigation.page == Page::Translate {
            (self.translation_model(), self.translation.is_generating())
        } else {
            (
                self.current_model(),
                self.current_conversation().is_none() || self.is_current_generating(),
            )
        };
        if unavailable || model.is_none_or(|model| model.reasoning.is_none()) {
            return;
        }
        self.chat.text_selection.clear(window);
        self.overlays.response_model_turn_id = None;
        let delegate = ReasoningPickerDelegate::from_app(self);
        let selected = delegate.initial_selection();
        self.overlays.reasoning_picker.update(cx, |picker, cx| {
            *picker.delegate_mut() = delegate;
            picker.set_query("", window, cx);
            picker.set_selected_index(selected, window, cx);
            picker.scroll_to_selected_item(window, cx);
        });

        self.open_shell_overlay(ShellOverlay::ReasoningPicker, true, window, cx);
        self.overlays
            .reasoning_picker
            .update(cx, |picker, cx| picker.focus(window, cx));
    }

    pub(crate) fn open_prompt_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.current_conversation().is_none() || self.is_current_generating() {
            return;
        }
        self.chat.text_selection.clear(window);
        self.overlays.response_model_turn_id = None;
        let delegate = PromptPickerDelegate::from_app(self);
        let selected = delegate.initial_selection();
        self.overlays.prompt_picker.update(cx, |picker, cx| {
            *picker.delegate_mut() = delegate;
            picker.set_query("", window, cx);
            picker.set_selected_index(selected, window, cx);
            picker.scroll_to_selected_item(window, cx);
        });

        self.open_shell_overlay(ShellOverlay::PromptPicker, true, window, cx);
        self.overlays
            .prompt_picker
            .update(cx, |picker, cx| picker.focus(window, cx));
        self.reload_snapshot(cx);
    }

    pub(super) fn open_shell_overlay(
        &mut self,
        overlay: ShellOverlay,
        animated: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.overlays.active.is_none() {
            self.overlays.previous_focus = window.focused(cx);
        }
        self.overlays.active = Some(overlay);
        if animated {
            self.overlays.motion.set_visible(true);
        } else {
            self.overlays.motion.snap_visible(true);
        }
        cx.notify();
    }

    pub(crate) fn close_shell_overlay(&mut self, animated: bool, cx: &mut Context<Self>) {
        self.overlays.response_model_turn_id = None;
        if animated {
            self.overlays.motion.set_visible(false);
        } else {
            self.overlays.motion.snap_visible(false);
        }
        cx.notify();
    }

    pub(crate) fn close_shell_overlay_immediate(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_shell_overlay(false, cx);
        self.overlays.active = None;
        if let Some(focus) = self.overlays.previous_focus.take() {
            window.focus(&focus, cx);
        }
    }

    pub(crate) fn select_prompt_preset(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        if self.is_current_generating() {
            return;
        }
        let Some(mut conversation) = self.current_conversation().cloned() else {
            return;
        };
        let (system_prompt, assistant_opening) = match name {
            Some(name) => {
                let Some(preset) = self.prompt_preset(&name) else {
                    self.data.error = Some(format!("Prompt preset not found: {name}"));
                    cx.notify();
                    return;
                };
                (
                    preset.system_prompt.clone(),
                    preset.assistant_opening.clone(),
                )
            }
            None => (String::new(), String::new()),
        };
        if conversation.system_prompt == system_prompt
            && conversation.assistant_opening == assistant_opening
        {
            cx.notify();
            return;
        }
        conversation.system_prompt = system_prompt;
        conversation.assistant_opening = assistant_opening;
        conversation.updated_at = now_timestamp();
        self.save_conversation_update(conversation, cx);
    }

    pub(crate) fn open_prompt_settings(&mut self, cx: &mut Context<Self>) {
        self.set_page(Page::Settings, cx);
        self.settings_ui.section = SettingsSection::SystemPrompts;
        self.reload_snapshot(cx);
        cx.notify();
    }

    pub(crate) fn select_model(&mut self, model_id: String, cx: &mut Context<Self>) {
        self.cancel_voice_recording(cx);
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
        if self.navigation.page == Page::Translate {
            self.translation.reasoning_preset = model
                .reasoning
                .as_ref()
                .map(|reasoning| reasoning.default_preset().to_string());
            self.translation.model_id = Some(model.id);
            cx.notify();
            return;
        }

        let conversation = self.current_conversation().cloned();
        let response_turn_id = self.overlays.response_model_turn_id.take();

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
        self.save_conversation_update(conversation, cx);
    }
}
