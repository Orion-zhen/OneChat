use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use gpui::{Context, ScrollHandle, Task, prelude::*};
use tokio_util::sync::CancellationToken;

use super::super::{CachedMarkdown, OneChat, UnseenGeneration};
use crate::{
    application::{
        generation::{GenerationStart, GenerationUpdate, PreparedGeneration, run_generation},
        prompt::PromptContext,
    },
    domain::{AssistantBlock, AssistantResponse, MessageStatus, RequestInfo},
    markdown::MarkdownDocument,
};

impl OneChat {
    pub(in crate::desktop::app) fn begin_prepared_generation(
        &mut self,
        mut prepared: PreparedGeneration,
        cx: &mut Context<Self>,
    ) {
        self.cancel_voice_recording(cx);
        let conversation_id = prepared.request_info.conversation_id.clone();
        let prompt_context = PromptContext {
            conversation_id: conversation_id.clone(),
            conversation_title: self
                .data
                .snapshot
                .conversations
                .iter()
                .find(|conversation| conversation.id == conversation_id)
                .map_or_else(String::new, |conversation| conversation.title.clone()),
            model_name: prepared.provider_request.model.display_name.clone(),
            provider_name: prepared.provider_request.provider.name.clone(),
        };
        prepared.configure_prompt(
            self.data.snapshot.settings.prompt_variables.clone(),
            prompt_context,
        );
        if self.chat.generations.is_active(&conversation_id) {
            self.data.error = Some("This conversation already has an active generation.".into());
            cx.notify();
            return;
        }
        let cancellation = CancellationToken::new();
        if !self.chat.generations.start(
            conversation_id.clone(),
            prepared.request_info.id.clone(),
            prepared.response.id.clone(),
            cancellation.clone(),
        ) {
            return;
        }
        self.sidebar.unseen_generations.remove(&conversation_id);

        self.chat.history_limit_preview = None;
        let turn_id = prepared.request_info.turn_id.clone();
        let response_id = prepared.response.id.clone();
        let scroll_to_bottom =
            matches!(&prepared.start, GenerationStart::NewTurn(_)) || self.is_latest_turn(&turn_id);
        if !matches!(&prepared.start, GenerationStart::NewTurn(_)) {
            self.chat
                .visible_response_ids
                .insert(turn_id, response_id.clone());
        }
        if scroll_to_bottom {
            self.chat.message_scroll_motion.cancel();
            self.chat.follow_latest = true;
            self.chat.message_scroll.scroll_to_bottom();
        }
        self.chat.message_editor = None;
        self.chat.thinking_expansion_overrides.remove(&response_id);
        self.chat.thinking_motions.remove(&response_id);
        self.chat
            .thinking_scrolls
            .insert(response_id, ScrollHandle::new());
        cx.notify();

        let persisted = prepared.clone();
        let storage = self.services.storage.clone();
        let previous = std::mem::replace(&mut self.data.storage_task, Task::ready(()));
        self.data.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move {
                    let persistence = match &persisted.start {
                        GenerationStart::NewTurn(turn) => {
                            storage.begin_turn(turn, &persisted.request_info)
                        }
                        GenerationStart::AddResponse { turn_id } => storage.begin_response(
                            &persisted.request_info.conversation_id,
                            turn_id,
                            &persisted.response,
                            &persisted.request_info,
                        ),
                        GenerationStart::RetryResponse { turn_id } => storage.begin_regeneration(
                            &persisted.request_info.conversation_id,
                            turn_id,
                            &persisted.response,
                            &persisted.request_info,
                        ),
                    };
                    if let Err(error) = persistence {
                        let _ = storage.remove_attachments(
                            &persisted.request_info.conversation_id,
                            &persisted.new_attachments,
                        );
                        return Err(error);
                    }
                    storage.load_snapshot()
                })
                .await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(snapshot) => {
                    this.data.snapshot = snapshot;
                    this.data.error = None;
                    this.chat.selected_request_id = Some(prepared.request_info.id.clone());
                    this.refresh_markdown_documents(cx);
                    this.launch_generation(prepared, cancellation, cx);
                    cx.notify();
                }
                Err(error) => {
                    this.chat
                        .generations
                        .finish(&conversation_id, &prepared.request_info.id);
                    this.data.error = Some(format!("Could not start generation: {error}"));
                    cx.notify();
                }
            });
        });
    }

    fn launch_generation(
        &mut self,
        prepared: PreparedGeneration,
        cancellation: CancellationToken,
        cx: &mut Context<Self>,
    ) {
        let conversation_id = prepared.request_info.conversation_id.clone();
        let request_id = prepared.request_info.id.clone();
        let root_user_message = self
            .data
            .snapshot
            .current_turns
            .iter()
            .find(|turn| turn.id == prepared.request_info.turn_id)
            .filter(|turn| turn.parent_response_id.is_none())
            .map(|turn| turn.user.clone());
        self.chat
            .thinking_started_at
            .insert(request_id.clone(), Instant::now());
        let storage = self.services.storage.clone();
        let mcp = self.services.mcp.clone();
        let (sender, receiver) = async_channel::bounded(32);
        self.services
            .runtime
            .spawn(run_generation(prepared, storage, mcp, cancellation, sender));

        let timer_request_id = request_id.clone();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let ticking = this
                    .update(cx, |this, cx| {
                        let ticking = this
                            .chat
                            .thinking_started_at
                            .contains_key(&timer_request_id);
                        if ticking {
                            cx.notify();
                        }
                        ticking
                    })
                    .unwrap_or(false);
                if !ticking {
                    break;
                }
            }
        })
        .detach();

        let cleanup_request_id = request_id.clone();
        cx.spawn(async move |this, cx| {
            let mut last_markdown_sources = HashMap::<String, String>::new();
            while let Ok(update) = receiver.recv().await {
                match update {
                    GenerationUpdate::PersistenceFailed(error) => {
                        let _ = this.update(cx, |this, cx| {
                            this.data.error = Some(format!("Could not save generation: {error}"));
                            cx.notify();
                        });
                    }
                    GenerationUpdate::Snapshot(snapshot) => {
                        let response = snapshot.response;
                        let request = snapshot.request;
                        let terminal = snapshot.terminal;
                        let finished_reasoning_ids = snapshot.finished_reasoning_ids;
                        let markdown_sources = response_output_sources(&response)
                            .into_iter()
                            .filter_map(|(id, source)| {
                                if last_markdown_sources.get(&id) == Some(&source) {
                                    None
                                } else {
                                    last_markdown_sources.insert(id.clone(), source.clone());
                                    Some((id, source))
                                }
                            })
                            .collect::<Vec<_>>();
                        let parsed_markdown = cx
                            .background_spawn(async move {
                                markdown_sources
                                    .into_iter()
                                    .map(|(id, source)| {
                                        let document = MarkdownDocument::parse(&source);
                                        (id, source, document)
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .await;
                        let _ = this.update(cx, |this, cx| {
                            if terminal {
                                this.chat.thinking_started_at.remove(&request.id);
                            }
                            let visible = this.update_generation_snapshot(
                                &conversation_id,
                                &response,
                                &request,
                            );
                            if visible {
                                for reasoning_id in finished_reasoning_ids {
                                    this.finish_thinking(reasoning_id);
                                }
                            }
                            for (id, source, document) in parsed_markdown {
                                let current = this
                                    .response(&response.id)
                                    .and_then(|(_, stored)| response_output_source(stored, &id))
                                    == Some(source.as_str());
                                if current {
                                    this.chat
                                        .markdown_documents
                                        .insert(id, CachedMarkdown { source, document });
                                }
                            }
                            if terminal {
                                this.chat.generations.finish(&conversation_id, &request_id);
                                let completed_in_background = response.status
                                    == MessageStatus::Completed
                                    && this
                                        .data
                                        .snapshot
                                        .settings
                                        .current_conversation_id
                                        .as_deref()
                                        != Some(conversation_id.as_str());
                                if completed_in_background {
                                    let completion_phase = this
                                        .sidebar
                                        .generation_border_clock(&conversation_id)
                                        .phase();
                                    this.sidebar.unseen_generations.insert(
                                        conversation_id.clone(),
                                        UnseenGeneration {
                                            request_id: request_id.clone(),
                                            completion_phase,
                                        },
                                    );
                                }
                                if let Some(user_message) = root_user_message.clone() {
                                    this.start_auto_title(
                                        conversation_id.clone(),
                                        user_message,
                                        &response,
                                        cx,
                                    );
                                }
                            }
                            cx.notify();
                        });
                        if terminal {
                            break;
                        }
                    }
                }
            }
            let _ = this.update(cx, |this, _| {
                this.chat.thinking_started_at.remove(&cleanup_request_id);
            });
        })
        .detach();
    }

    fn update_generation_snapshot(
        &mut self,
        conversation_id: &str,
        response: &AssistantResponse,
        request: &RequestInfo,
    ) -> bool {
        if self
            .data
            .snapshot
            .settings
            .current_conversation_id
            .as_deref()
            != Some(conversation_id)
        {
            return false;
        }
        let thinking_grew = self
            .response(&response.id)
            .is_none_or(|(_, stored)| stored.thinking.len() < response.thinking.len());
        if let Some(turn) = self
            .data
            .snapshot
            .current_turns
            .iter_mut()
            .find(|turn| turn.id == request.turn_id)
        {
            if let Some(stored) = turn
                .responses
                .iter_mut()
                .find(|stored| stored.id == response.id)
            {
                *stored = response.clone();
            }
            turn.promote_continuation_response(&response.id);
        }
        if let Some(info) = self
            .data
            .snapshot
            .current_requests
            .iter_mut()
            .find(|info| info.id == request.id)
        {
            *info = request.clone();
        }
        if thinking_grew {
            let reasoning_id = response
                .blocks
                .iter()
                .rev()
                .find_map(|block| match block {
                    AssistantBlock::Reasoning { id, .. } => Some(id.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| response.id.clone());
            self.chat
                .thinking_scrolls
                .entry(reasoning_id)
                .or_default()
                .scroll_to_bottom();
        }
        if self.chat.follow_latest && self.is_latest_turn(&request.turn_id) {
            self.chat.message_scroll.scroll_to_bottom();
        }
        true
    }
}

fn response_output_sources(response: &AssistantResponse) -> Vec<(String, String)> {
    if response.blocks.is_empty() {
        return vec![(response.id.clone(), response.content.clone())];
    }
    response
        .blocks
        .iter()
        .filter_map(|block| match block {
            AssistantBlock::Output { id, content } => Some((id.clone(), content.clone())),
            _ => None,
        })
        .collect()
}

fn response_output_source<'a>(response: &'a AssistantResponse, id: &str) -> Option<&'a str> {
    if response.blocks.is_empty() {
        return (response.id == id).then_some(response.content.as_str());
    }
    response.blocks.iter().find_map(|block| match block {
        AssistantBlock::Output {
            id: output_id,
            content,
        } if output_id == id => Some(content.as_str()),
        _ => None,
    })
}
