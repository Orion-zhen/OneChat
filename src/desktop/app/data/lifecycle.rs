use std::collections::HashMap;

use gpui::{Context, ScrollHandle, Task, Window, prelude::*};
use gpui_component::select::SelectEvent;

use super::super::{
    CachedMarkdown, OneChat, Page, PendingFocus, SystemPromptMode, TitleTransition,
};
use crate::{
    desktop::ui::inspector::{
        GenerationConfigEditor, GenerationParameterItem, ReasoningPresetItem,
    },
    domain::{AssistantBlock, AutoTitleState},
    markdown::MarkdownDocument,
    storage::{Storage, StorageResult, StorageSnapshot},
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
        let transient_session = self
            .chat
            .transient_conversation_id
            .as_deref()
            .and_then(|id| {
                self.data
                    .snapshot
                    .conversations
                    .iter()
                    .find(|conversation| conversation.id == id)
            })
            .cloned()
            .map(|conversation| {
                (
                    conversation,
                    self.data.snapshot.current_turns.clone(),
                    self.data.snapshot.current_requests.clone(),
                )
            });
        match result {
            Ok(mut snapshot) => {
                if let Some((conversation, turns, requests)) = transient_session {
                    snapshot.conversations.push(conversation);
                    snapshot.current_turns = turns;
                    snapshot.current_requests = requests;
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
                let completed_title_transitions = snapshot
                    .conversations
                    .iter()
                    .filter_map(|conversation| {
                        let pending = self.chat.pending_title_transitions.get(&conversation.id)?;
                        (conversation.auto_title_state == AutoTitleState::Finished).then(|| {
                            (
                                conversation.id.clone(),
                                (conversation.title == pending.new_title).then(|| {
                                    TitleTransition::new(&pending.old_title, &pending.new_title)
                                }),
                            )
                        })
                    })
                    .collect::<Vec<_>>();
                for (conversation_id, transition) in completed_title_transitions {
                    self.chat.pending_title_transitions.remove(&conversation_id);
                    if let Some(transition) = transition {
                        self.chat
                            .title_transitions
                            .insert(conversation_id, transition);
                    }
                }
                let stored_titles = snapshot
                    .conversations
                    .iter()
                    .map(|conversation| (conversation.id.clone(), conversation.title.clone()))
                    .collect::<HashMap<_, _>>();
                self.chat.pending_title_transitions.retain(|id, _| {
                    snapshot
                        .conversations
                        .iter()
                        .any(|conversation| &conversation.id == id)
                });
                self.chat.title_transitions.retain(|id, transition| {
                    stored_titles
                        .get(id)
                        .is_some_and(|title| title == &transition.new_title)
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
                let previous_conversation_id =
                    self.data.snapshot.settings.current_conversation_id.clone();
                let conversation_changed =
                    previous_conversation_id != snapshot.settings.current_conversation_id;
                let translation_used_defaults = self.translation.uses_default_prompts(
                    &self.data.snapshot.settings.translation_system_prompt,
                    &self.data.snapshot.settings.translation_user_prompt,
                );
                self.data.snapshot = snapshot;
                if translation_used_defaults {
                    self.translation.system_prompt = self
                        .data
                        .snapshot
                        .settings
                        .translation_system_prompt
                        .clone();
                    self.translation.user_prompt =
                        self.data.snapshot.settings.translation_user_prompt.clone();
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
                self.sync_thinking_scrolls();
                self.sync_tool_execution_expansions();
                self.refresh_markdown_documents(cx);
            }
            Err(error) => {
                self.chat.pending_search_target = None;
                self.data.error = Some(format!("Storage error: {error}"));
            }
        }
    }

    pub(in crate::desktop::app) fn refresh_markdown_documents(&mut self, cx: &mut Context<Self>) {
        let sources = markdown_sources(&self.data.snapshot, self.current_conversation_id());
        self.chat.markdown_documents.retain(|id, cached| {
            sources
                .get(id)
                .is_some_and(|source| source == &cached.source)
        });
        let pending = sources
            .into_iter()
            .filter(|(id, _)| !self.chat.markdown_documents.contains_key(id))
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
        self.chat.draft_model_id = None;
        self.chat.system_prompt_mode = SystemPromptMode::Compact;
        self.chat.system_prompt_editor = None;
        self.chat.assistant_opening_editor = None;
        self.chat.selected_request_id = None;
        self.chat.visible_response_ids.clear();
        self.overlays.response_model_turn_id = None;
        self.chat.expanded_error_ids.clear();
        self.chat.thinking_expansion_overrides.clear();
        self.chat.expanded_tool_execution_ids.clear();
        self.chat.expanded_conversation_tool_server_ids.clear();
        self.chat.message_editor = None;
        self.chat.branch_swipe.reset();
        #[cfg(target_os = "macos")]
        self.chat.response_tab_force_click.cancel();
        self.chat.horizontal_scrolls.clear();
        self.chat.follow_latest = true;
        self.chat.message_scroll_motion.cancel();
        self.chat.message_scroll = ScrollHandle::new();
        self.chat.message_scroll.scroll_to_bottom();
        self.chat.timeline.hovered = false;
        self.chat.timeline.pointer_y = None;
        self.chat.timeline.active_item = None;
        self.chat.timeline.expansion_motion.set_visible(false);
        self.chat.thinking_scrolls.clear();
        self.chat.thinking_motions.clear();
        self.chat.generation_config_editor = None;
        self.chat.history_limit_preview = None;
        self.chat.generation_config_save_revision =
            self.chat.generation_config_save_revision.wrapping_add(1);
        self.chat.parameter_error = None;
        self.chat.attachments.clear();
        self.chat.attachment_previews.clear();
        self.chat.temporary_attachment_files.clear();
        self.chat.attachments_loading = false;
        self.chat.attachments_revision = self.chat.attachments_revision.wrapping_add(1);
    }

    fn sync_tool_execution_expansions(&mut self) {
        self.chat.expanded_tool_execution_ids.retain(|id| {
            self.data
                .snapshot
                .current_turns
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
            .current_turns
            .iter()
            .flat_map(|turn| &turn.responses)
            .flat_map(|response| {
                if response.blocks.is_empty() {
                    vec![response.id.clone()]
                } else {
                    response
                        .blocks
                        .iter()
                        .filter_map(|block| match block {
                            AssistantBlock::Reasoning { id, .. } => Some(id.clone()),
                            _ => None,
                        })
                        .collect()
                }
            })
            .collect::<std::collections::HashSet<_>>();
        self.chat
            .thinking_motions
            .retain(|reasoning_id, _| reasoning_ids.contains(reasoning_id));
        self.chat
            .thinking_scrolls
            .retain(|reasoning_id, _| reasoning_ids.contains(reasoning_id));
        for reasoning_id in reasoning_ids {
            self.chat.thinking_scrolls.entry(reasoning_id).or_default();
        }
    }

    pub(in crate::desktop::app) fn reload_snapshot(&mut self, cx: &mut Context<Self>) {
        let previous = std::mem::replace(&mut self.data.storage_task, Task::ready(()));
        let storage = self.services.storage.clone();
        self.data.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move { storage.load_snapshot() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.apply_snapshot(result, cx);
                cx.notify();
            });
        });
    }

    pub(in crate::desktop::app) fn mutate_and_reload<F>(
        &mut self,
        operation: F,
        cx: &mut Context<Self>,
    ) where
        F: FnOnce(&Storage) -> StorageResult<()> + Send + 'static,
    {
        let previous = std::mem::replace(&mut self.data.storage_task, Task::ready(()));
        let storage = self.services.storage.clone();
        self.data.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move {
                    operation(&storage)?;
                    storage.load_snapshot()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.apply_snapshot(result, cx);
                cx.notify();
            });
        });
    }

    pub(in crate::desktop::app) fn save_settings(&mut self, cx: &mut Context<Self>) {
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
    for turn in &snapshot.current_turns {
        sources.insert(turn.user.id.clone(), turn.user.content.clone());
        for response in &turn.responses {
            if response.blocks.is_empty() {
                sources.insert(response.id.clone(), response.content.clone());
                continue;
            }
            for block in &response.blocks {
                if let AssistantBlock::Output { id, content } = block {
                    sources.insert(id.clone(), content.clone());
                }
            }
        }
    }
    sources
}
