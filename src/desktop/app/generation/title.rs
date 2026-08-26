use gpui::Context;

use super::super::{OneChat, PendingTitleTransition};
use crate::{
    application::title::generate_title,
    domain::{
        AppSettings, AssistantResponse, AutoTitleState, Conversation, Model, TitleModelSource,
        UserMessage,
    },
};

enum AutoTitleRequest {
    Initial {
        user_message: UserMessage,
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
        user_message: UserMessage,
        response: &AssistantResponse,
        cx: &mut Context<Self>,
    ) {
        if !response.is_usable_as_context() || response.content.trim().is_empty() {
            return;
        }
        if !self.data.snapshot.conversations.iter().any(|conversation| {
            conversation.id == conversation_id
                && !conversation.temporary
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
        if !self.data.snapshot.settings.auto_title_enabled
            || self
                .data
                .snapshot
                .conversations
                .iter()
                .any(|conversation| conversation.id == conversation_id && conversation.temporary)
        {
            return;
        }

        let system_prompt = self
            .data
            .snapshot
            .settings
            .title_generation_system_prompt
            .trim()
            .to_string();
        let target = self
            .data
            .snapshot
            .conversations
            .iter()
            .find(|conversation| conversation.id == conversation_id)
            .and_then(|conversation| {
                resolve_title_target(
                    &self.data.snapshot.settings,
                    conversation,
                    &self.data.snapshot.models,
                )
            })
            .filter(|(model, _)| self.model_availability(model).is_ok())
            .and_then(|(model, reasoning_preset)| {
                self.provider_for_model(model)
                    .map(|provider| (provider.clone(), model.clone(), reasoning_preset))
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
                    .map(|claimed| claimed.then_some(vec![(user_message, assistant_response)])),
                AutoTitleRequest::Regenerate => storage.restart_auto_title(&claim_id),
            })
            .await;
            let conversation = match claimed {
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
                Some((provider, model, reasoning_preset)) => generate_title(
                    provider,
                    model,
                    system_prompt,
                    reasoning_preset,
                    conversation,
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

fn resolve_title_target<'a>(
    settings: &AppSettings,
    conversation: &Conversation,
    models: &'a [Model],
) -> Option<(&'a Model, Option<String>)> {
    let model = match &settings.title_generation_model {
        TitleModelSource::Current => conversation
            .model_id
            .as_deref()
            .and_then(|model_id| models.iter().find(|model| model.id == model_id))
            .or_else(|| {
                settings
                    .primary_model_id
                    .as_deref()
                    .and_then(|model_id| models.iter().find(|model| model.id == model_id))
            }),
        TitleModelSource::Primary => settings
            .primary_model_id
            .as_deref()
            .and_then(|model_id| models.iter().find(|model| model.id == model_id)),
        TitleModelSource::Model(model_id) => models.iter().find(|model| model.id == *model_id),
    }?;
    let reasoning_preset = match &settings.title_generation_model {
        TitleModelSource::Current => conversation.generation_config.reasoning_preset.clone(),
        TitleModelSource::Primary | TitleModelSource::Model(_) => {
            settings.title_generation_reasoning_preset.clone()
        }
    };
    Some((model, reasoning_preset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Provider, ProviderKind};

    #[test]
    fn current_source_uses_the_target_conversations_model_and_reasoning() {
        let provider = Provider::new("Provider", ProviderKind::OpenAi);
        let primary = Model::new(&provider.id, "primary", "Primary");
        let current = Model::new(&provider.id, "current", "Current");
        let mut conversation = Conversation::new("Chat", Some(&current), "");
        conversation.generation_config.reasoning_preset = Some("high".into());
        let settings = AppSettings {
            primary_model_id: Some(primary.id.clone()),
            title_generation_reasoning_preset: Some("low".into()),
            ..AppSettings::default()
        };

        let models = [primary, current.clone()];
        let (resolved, reasoning) =
            resolve_title_target(&settings, &conversation, &models).unwrap();

        assert_eq!(resolved.id, current.id);
        assert_eq!(reasoning.as_deref(), Some("high"));
    }

    #[test]
    fn fixed_source_uses_the_title_reasoning_setting() {
        let provider = Provider::new("Provider", ProviderKind::OpenAi);
        let primary = Model::new(&provider.id, "primary", "Primary");
        let current = Model::new(&provider.id, "current", "Current");
        let mut conversation = Conversation::new("Chat", Some(&current), "");
        conversation.generation_config.reasoning_preset = Some("high".into());
        let settings = AppSettings {
            primary_model_id: Some(primary.id.clone()),
            title_generation_model: TitleModelSource::Primary,
            title_generation_reasoning_preset: Some("low".into()),
            ..AppSettings::default()
        };

        let models = [primary.clone(), current];
        let (resolved, reasoning) =
            resolve_title_target(&settings, &conversation, &models).unwrap();

        assert_eq!(resolved.id, primary.id);
        assert_eq!(reasoning.as_deref(), Some("low"));
    }
}
