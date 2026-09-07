use std::collections::HashMap;

use gpui::{Context, Task, Window, prelude::*};
use gpui_component::select::SelectEvent;

use super::super::{CachedMarkdown, OneChat, Page, PendingFocus};
use crate::{
    desktop::ui::inspector::{
        GenerationConfigEditor, GenerationParameterItem, ReasoningPresetItem,
    },
    markdown::MarkdownDocument,
    storage::{StorageResult, StorageSnapshot},
};

impl OneChat {
    pub(in crate::desktop::app) fn load_startup_snapshot(&mut self, cx: &mut Context<Self>) {
        let previous = std::mem::replace(&mut self.data.storage_task, Task::ready(()));
        let storage = self.services.storage.clone();
        self.data.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move { storage.load_startup_snapshot() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.data.loading = false;
                this.apply_snapshot(result, cx);
                cx.notify();
            });
        });
    }

    pub(in crate::desktop::app) fn apply_snapshot(
        &mut self,
        result: StorageResult<StorageSnapshot>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(mut snapshot) => {
                if self.chat.transient_conversation_id.is_some()
                    && let Some(session) = self.data.snapshot.current.take()
                {
                    let id = session.conversation.id.clone();
                    snapshot.apply_conversation(session, Some(&id));
                }
                if matches!(
                    self.navigation.page,
                    Page::Chat | Page::Translate | Page::Tts
                ) {
                    let width = if snapshot.settings.sidebar_collapsed {
                        0.0
                    } else {
                        self.sidebar.width
                    };
                    self.navigation
                        .sidebar_width_motion
                        .set_target(width, false);
                }
                for conversation in &snapshot.conversations {
                    self.update_title_transition(conversation);
                }
                self.chat.pending_title_transitions.retain(|id, _| {
                    snapshot
                        .conversations
                        .iter()
                        .any(|conversation| &conversation.id == id)
                });
                self.chat.title_transitions.retain(|id, _| {
                    snapshot
                        .conversations
                        .iter()
                        .any(|conversation| &conversation.id == id)
                });

                for conversation in &mut snapshot.conversations {
                    if let Some(current) = self
                        .data
                        .snapshot
                        .conversations
                        .iter()
                        .find(|current| current.id == conversation.id)
                    {
                        conversation.auto_title_state =
                            conversation.auto_title_state.max(current.auto_title_state);
                    }
                }
                if let Some(current) = snapshot.current.as_mut()
                    && let Some(conversation) = snapshot
                        .conversations
                        .iter()
                        .find(|conversation| conversation.id == current.conversation.id)
                {
                    current.conversation.clone_from(conversation);
                }
                let previous_conversation_id =
                    self.data.snapshot.settings.current_conversation_id.clone();
                let conversation_changed =
                    previous_conversation_id != snapshot.settings.current_conversation_id;
                let translation_used_defaults = self.translation.uses_default_prompts(
                    &self.data.snapshot.settings.translation_system_prompt,
                    &self.data.snapshot.settings.translation_user_prompt,
                );
                self.data.snapshot = snapshot;
                self.settings_ui.controls_dirty = true;
                self.chat.controls_dirty = true;
                if translation_used_defaults {
                    self.set_translation_prompts(
                        self.settings().translation_system_prompt.clone(),
                        self.settings().translation_user_prompt.clone(),
                        cx,
                    );
                }
                self.settings_ui.history_limit_save_pending = false;
                self.chat.history_limit_preview = None;
                self.data.error = None;
                if conversation_changed {
                    self.reset_conversation_ui(cx);
                    if self.current_conversation().is_some() {
                        self.navigation.pending_focus = Some(PendingFocus::Composer);
                    }
                }
                self.refresh_conversation_content(cx);
            }
            Err(error) => {
                self.chat.pending_search_target = None;
                self.data.error = Some(format!("Storage error: {error}"));
            }
        }
    }

    pub(in crate::desktop::app) fn refresh_conversation_content(&mut self, cx: &mut Context<Self>) {
        self.chat.controls_dirty = true;
        self.sync_thinking_scrolls();
        self.sync_tool_execution_expansions();
        self.refresh_markdown_documents(cx);
    }

    pub(in crate::desktop::app) fn refresh_markdown_documents(&mut self, cx: &mut Context<Self>) {
        let sources = markdown_sources(&self.data.snapshot, self.current_conversation_id());
        self.chat
            .presentation
            .markdown_documents
            .retain(|id, cached| {
                sources
                    .get(id)
                    .is_some_and(|source| source == &cached.source)
            });
        let pending = sources
            .into_iter()
            .filter(|(id, _)| !self.chat.presentation.markdown_documents.contains_key(id))
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let parsed = cx
                .background_spawn(async move {
                    pending
                        .into_iter()
                        .map(|(id, source)| {
                            let document = MarkdownDocument::parse(&source);
                            (id, source, document)
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let current = markdown_sources(&this.data.snapshot, this.current_conversation_id());
                for (id, source, document) in parsed {
                    if current.get(&id) == Some(&source) {
                        this.chat
                            .presentation
                            .markdown_documents
                            .insert(id, CachedMarkdown { source, document });
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn markdown_for(&self, message_id: &str, source: &str) -> Option<&MarkdownDocument> {
        self.chat
            .presentation
            .markdown_documents
            .get(message_id)
            .filter(|cached| cached.source == source)
            .map(|cached| &cached.document)
    }

    pub(crate) fn sync_generation_config_editor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let conversation = self.current_conversation().cloned();
        match conversation {
            Some(conversation)
                if self
                    .chat
                    .generation_config_editor
                    .as_ref()
                    .is_none_or(|editor| !editor.is_for(&conversation.id)) =>
            {
                let editor = GenerationConfigEditor::new(&conversation, window, cx);
                let parameter_select = editor.parameter_select.clone();
                let reasoning_select = editor.reasoning_select.clone();
                self.chat.generation_config_editor = Some(editor);
                cx.subscribe_in(
                    &parameter_select,
                    window,
                    |this,
                     select,
                     event: &SelectEvent<Vec<GenerationParameterItem>>,
                     window,
                     cx| {
                        let SelectEvent::Confirm(Some(parameter)) = event else {
                            return;
                        };
                        this.add_generation_parameter(*parameter, cx);
                        select.update(cx, |select, cx| select.set_selected_index(None, window, cx));
                    },
                )
                .detach();
                cx.subscribe_in(
                    &reasoning_select,
                    window,
                    |this, _, event: &SelectEvent<Vec<ReasoningPresetItem>>, _, cx| {
                        let SelectEvent::Confirm(Some(preset)) = event else {
                            return;
                        };
                        this.select_reasoning_preset(preset.clone(), cx);
                    },
                )
                .detach();
                self.chat.parameter_error = None;
            }
            None => {
                self.chat.generation_config_editor = None;
                self.chat.parameter_error = None;
            }
            Some(_) => {}
        }
    }

    pub(in crate::desktop::app) fn reset_conversation_ui(&mut self, cx: &mut Context<Self>) {
        self.cancel_voice_recording(cx);
        self.stop_audio_playback();
        self.overlays.response_model_turn_id = None;
        self.chat.reset_conversation();
    }

    fn sync_tool_execution_expansions(&mut self) {
        self.chat.expanded_tool_execution_ids.retain(|id| {
            self.data
                .snapshot
                .current_turns()
                .iter()
                .flat_map(|turn| &turn.responses)
                .flat_map(|response| &response.tool_executions)
                .any(|execution| execution.id == *id)
        });
    }

    fn sync_thinking_scrolls(&mut self) {
        let reasoning_ids = self
            .data
            .snapshot
            .current_turns()
            .iter()
            .flat_map(|turn| &turn.responses)
            .flat_map(|response| response.reasoning_blocks().map(|(id, _)| id.to_string()))
            .collect::<std::collections::HashSet<_>>();
        self.chat
            .presentation
            .thinking_motions
            .retain(|reasoning_id, _| reasoning_ids.contains(reasoning_id));
        self.chat
            .presentation
            .thinking_scrolls
            .retain(|reasoning_id, _| reasoning_ids.contains(reasoning_id));
        for reasoning_id in reasoning_ids {
            self.chat
                .presentation
                .thinking_scrolls
                .entry(reasoning_id)
                .or_default();
        }
    }

    pub(in crate::desktop::app) fn save_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.controls_dirty = true;
        let previous = std::mem::replace(&mut self.data.storage_task, Task::ready(()));
        let storage = self.services.storage.clone();
        let settings = self.data.snapshot.settings.clone();
        self.data.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move { storage.save_settings(&settings) })
                .await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.data.error = Some(format!("Could not save settings: {error}"));
                    cx.notify();
                });
            }
        });
    }
}

fn markdown_sources(
    snapshot: &StorageSnapshot,
    current_conversation_id: Option<&str>,
) -> HashMap<String, String> {
    let mut sources = HashMap::new();
    if let Some(conversation_id) = current_conversation_id
        && let Some(conversation) = snapshot
            .conversations
            .iter()
            .find(|conversation| conversation.id == conversation_id)
        && !conversation.assistant_opening.is_empty()
    {
        sources.insert(
            format!("assistant-opening-{}", conversation.id),
            conversation.assistant_opening.clone(),
        );
    }
    for turn in snapshot.current_turns() {
        sources.insert(turn.user.id.clone(), turn.user.content.clone());
        for response in &turn.responses {
            sources.extend(
                response
                    .output_blocks()
                    .map(|(id, content)| (id.to_string(), content.to_string())),
            );
        }
    }
    sources
}
