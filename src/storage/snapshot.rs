use crate::domain::{
    AutoTitleState, MessageStatus, RequestStatus, ToolExecutionStatus, now_timestamp,
};

use super::{Result, Storage, StorageSnapshot};

impl Storage {
    pub(super) fn load_snapshot_locked(&self) -> Result<StorageSnapshot> {
        let mut settings = self.read_settings()?;
        let files = self.read_conversations()?;
        let prompt_presets = self.read_prompt_presets()?;
        let mut settings_changed = settings.app.normalize_fonts();
        if settings
            .app
            .current_conversation_id
            .as_ref()
            .is_some_and(|id| !files.iter().any(|file| &file.conversation.id == id))
        {
            settings.app.current_conversation_id = None;
            settings_changed = true;
        }
        if settings
            .app
            .primary_model_id
            .as_ref()
            .is_some_and(|id| !settings.models.iter().any(|model| &model.id == id))
        {
            settings.app.primary_model_id = None;
            settings_changed = true;
        }
        if settings
            .app
            .title_generation_model_id
            .as_ref()
            .is_some_and(|id| !settings.models.iter().any(|model| &model.id == id))
        {
            settings.app.title_generation_model_id = None;
            settings_changed = true;
        }
        if settings_changed {
            self.write_settings(&settings)?;
        }

        let (current_turns, current_requests) = settings
            .app
            .current_conversation_id
            .as_deref()
            .and_then(|id| files.iter().find(|file| file.conversation.id == id))
            .map(|file| {
                let turns = file.turns.clone();
                let mut requests = file.requests.clone();
                requests.sort_by(|a, b| {
                    b.started_at
                        .cmp(&a.started_at)
                        .then_with(|| b.id.cmp(&a.id))
                });
                (turns, requests)
            })
            .unwrap_or_default();

        let providers = settings.providers;
        let mut models = settings.models;
        models.sort_by(|a, b| {
            a.display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        let mut conversations = files
            .into_iter()
            .map(|file| file.conversation)
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
            current_turns,
            current_requests,
            settings: settings.app,
        })
    }

    pub(super) fn recover_interrupted_locked(&self) -> Result<()> {
        for mut file in self.read_conversations()? {
            let mut changed = false;
            if file.conversation.auto_title_state == AutoTitleState::Running {
                file.conversation.auto_title_state = AutoTitleState::Finished;
                changed = true;
            }
            for response in file.turns.iter_mut().flat_map(|turn| &mut turn.responses) {
                if matches!(
                    response.status,
                    MessageStatus::Pending | MessageStatus::Streaming
                ) {
                    response.status = MessageStatus::Interrupted;
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
                self.write_conversation(&file)?;
            }
        }
        Ok(())
    }
}
