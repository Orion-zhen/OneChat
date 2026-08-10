use gpui::Context;

use super::super::{OneChat, PendingTitleTransition};
use crate::{
    application::title::generate_title,
    domain::{AssistantResponse, AutoTitleState, MessageStatus},
};

enum AutoTitleRequest {
    Initial {
        user_message: String,
        assistant_response: String,
    },
    Regenerate,
}

enum AutoTitleUpdate {
    Claimed,
    Finished(Option<String>),
    ClaimFailed(String),
}

impl OneChat {
    pub(super) fn start_auto_title(
        &mut self,
        conversation_id: String,
        user_message: String,
        response: &AssistantResponse,
        cx: &mut Context<Self>,
    ) {
        if response.status != MessageStatus::Completed || response.content.trim().is_empty() {
            return;
        }
        if !self.data.snapshot.conversations.iter().any(|conversation| {
            conversation.id == conversation_id
                && conversation.auto_title_state == AutoTitleState::Pending
        }) {
            return;
        }
        self.run_auto_title(
            conversation_id,
            AutoTitleRequest::Initial {
                user_message,
                assistant_response: response.content.clone(),
            },
            cx,
        );
    }

    pub(crate) fn regenerate_auto_title(
        &mut self,
        conversation_id: String,
        cx: &mut Context<Self>,
    ) {
        self.run_auto_title(conversation_id, AutoTitleRequest::Regenerate, cx);
    }

    fn run_auto_title(
        &mut self,
        conversation_id: String,
        request: AutoTitleRequest,
        cx: &mut Context<Self>,
    ) {
        if !self.data.snapshot.settings.auto_title_enabled {
            return;
        }

        let system_prompt = self
            .data
            .snapshot
            .settings
            .title_generation_system_prompt
            .trim()
            .to_string();
        let reasoning_preset = self
            .data
            .snapshot
            .settings
            .title_generation_reasoning_preset
            .clone();
        let target = self
            .title_generation_model()
            .filter(|model| self.model_availability(model).is_ok())
            .and_then(|model| {
                self.provider_for_model(model)
                    .map(|provider| (provider.clone(), model.clone()))
            })
            .filter(|_| !system_prompt.is_empty());
        let storage = self.services.storage.clone();
        let claim_id = conversation_id.clone();
        let (sender, receiver) = async_channel::bounded::<AutoTitleUpdate>(2);
        self.services.runtime.spawn(async move {
            let claimed = tokio::task::spawn_blocking(move || match request {
                AutoTitleRequest::Initial {
                    user_message,
                    assistant_response,
                } => storage
                    .claim_auto_title(&claim_id)
                    .map(|claimed| claimed.then_some((user_message, assistant_response))),
                AutoTitleRequest::Regenerate => storage.restart_auto_title(&claim_id),
            })
            .await;
            let (user_message, assistant_response) = match claimed {
                Ok(Ok(Some(source))) => {
                    if sender.send(AutoTitleUpdate::Claimed).await.is_err() {
                        return;
                    }
                    source
                }
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    let _ = sender
                        .send(AutoTitleUpdate::ClaimFailed(error.to_string()))
                        .await;
                    return;
                }
                Err(error) => {
                    let _ = sender
                        .send(AutoTitleUpdate::ClaimFailed(format!(
                            "automatic title claim task failed: {error}"
                        )))
                        .await;
                    return;
                }
            };

            let title = match target {
                Some((provider, model)) => generate_title(
                    provider,
                    model,
                    system_prompt,
                    reasoning_preset,
                    user_message,
                    assistant_response,
                )
                .await
                .ok(),
                None => None,
            };
            let _ = sender.send(AutoTitleUpdate::Finished(title)).await;
        });

        cx.spawn(async move |this, cx| {
            while let Ok(update) = receiver.recv().await {
                let finished = matches!(
                    &update,
                    AutoTitleUpdate::Finished(_) | AutoTitleUpdate::ClaimFailed(_)
                );
                let _ = this.update(cx, |this, cx| match update {
                    AutoTitleUpdate::Claimed => {
                        if let Some(conversation) = this
                            .data
                            .snapshot
                            .conversations
                            .iter_mut()
                            .find(|conversation| conversation.id == conversation_id)
                        {
                            conversation.auto_title_state = AutoTitleState::Running;
                            cx.notify();
                        }
                    }
                    AutoTitleUpdate::Finished(title) => {
                        let title = this
                            .data
                            .snapshot
                            .settings
                            .auto_title_enabled
                            .then_some(title)
                            .flatten();
                        if let Some(new_title) = title.as_ref()
                            && let Some(conversation) = this
                                .data
                                .snapshot
                                .conversations
                                .iter()
                                .find(|conversation| {
                                    conversation.id == conversation_id
                                        && conversation.auto_title_state == AutoTitleState::Running
                                })
                            && conversation.title != *new_title
                        {
                            this.chat.pending_title_transitions.insert(
                                conversation_id.clone(),
                                PendingTitleTransition {
                                    old_title: conversation.title.clone(),
                                    new_title: new_title.clone(),
                                },
                            );
                        }
                        let conversation_id = conversation_id.clone();
                        this.mutate_and_reload(
                            move |storage| {
                                storage
                                    .finish_auto_title(&conversation_id, title.as_deref())
                                    .map(|_| ())
                            },
                            cx,
                        );
                    }
                    AutoTitleUpdate::ClaimFailed(error) => {
                        this.data.error = Some(format!("Could not start automatic title: {error}"));
                        cx.notify();
                    }
                });
                if finished {
                    break;
                }
            }
        })
        .detach();
    }
}
