use std::collections::HashSet;

use crate::domain::{
    AutoTitleState, MessageStatus, RequestKind, RequestStatus, ToolExecutionStatus, now_timestamp,
};

use super::{
    ConversationSearchIndex, ModelCatalog, Result, Storage, StorageSnapshot, conversation::Sessions,
};

impl Storage {
    pub(super) fn snapshot(&self, sessions: &mut Sessions) -> Result<StorageSnapshot> {
        let mut settings = self.read_settings()?;
        let prompt_presets = self.read_prompt_presets()?;
        let mut settings_changed = settings.app.normalize();
        if settings
            .app
            .current_conversation_id
            .as_ref()
            .is_some_and(|id| !sessions.contains(id))
        {
            settings.app.current_conversation_id = None;
            settings_changed = true;
        }
        settings_changed |= settings.app.retain_models(&settings.models);
        if settings_changed {
            self.write_settings(&settings)?;
        }

        let mut current = settings
            .app
            .current_conversation_id
            .as_deref()
            .map(|id| self.session_for_use(sessions, id))
            .transpose()?;
        if let Some(session) = current.as_mut() {
            session.requests.sort_by(|a, b| {
                b.started_at
                    .cmp(&a.started_at)
                    .then_with(|| b.id.cmp(&a.id))
            });
        }

        let mut conversation_search = ConversationSearchIndex::default();
        for file in sessions.values() {
            conversation_search.insert_conversation(file.conversation.id.clone(), &file.turns);
        }

        let ModelCatalog { providers, models } =
            ModelCatalog::new(settings.providers, settings.models);
        let mut conversations = sessions
            .values()
            .map(|file| file.conversation.clone())
            .collect::<Vec<_>>();
        conversations.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.updated_at.cmp(&a.updated_at))
                .then_with(|| a.id.cmp(&b.id))
        });

        Ok(StorageSnapshot {
            providers,
            models,
            prompt_presets,
            conversations,
            conversation_search,
            current,
            settings: settings.app,
        })
    }

    pub(super) fn recover_interrupted_locked(&self, sessions: &mut Sessions) -> Result<()> {
        let ids = sessions
            .values()
            .map(|session| session.conversation.id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            let mut file = sessions.get(&id)?.clone();
            let mut changed = false;
            if file.conversation.auto_title_state == AutoTitleState::Running {
                file.conversation.auto_title_state = AutoTitleState::Finished;
                changed = true;
            }
            let interrupted_continuations = file
                .requests
                .iter()
                .filter(|request| {
                    request.kind == RequestKind::Continue
                        && matches!(
                            request.status,
                            RequestStatus::Sending | RequestStatus::Streaming
                        )
                })
                .map(|request| request.response_id.clone())
                .collect::<HashSet<_>>();
            for response in file.turns.iter_mut().flat_map(|turn| &mut turn.responses) {
                if matches!(
                    response.status,
                    MessageStatus::Pending | MessageStatus::Streaming
                ) {
                    if interrupted_continuations.contains(&response.id) && response.has_output() {
                        response.recover_interrupted_continuation();
                    } else {
                        response.status = MessageStatus::Interrupted;
                    }
                    changed = true;
                }
                for execution in &mut response.tool_executions {
                    if execution.status.is_active() {
                        execution.status = ToolExecutionStatus::Interrupted;
                        execution.finished_at = Some(now_timestamp());
                        changed = true;
                    }
                }
            }
            for request in &mut file.requests {
                if matches!(
                    request.status,
                    RequestStatus::Sending | RequestStatus::Streaming
                ) {
                    request.status = RequestStatus::Interrupted;
                    changed = true;
                }
            }
            if changed {
                self.commit_session(sessions, &file)?;
            }
        }
        Ok(())
    }
}
