use crate::domain::{MessageStatus, RequestStatus};

use super::{Result, Storage, StorageSnapshot};

impl Storage {
    pub(super) fn load_snapshot_locked(&self) -> Result<StorageSnapshot> {
        let mut settings = self.read_settings()?;
        let files = self.read_conversations()?;
        if settings
            .app
            .current_conversation_id
            .as_ref()
            .is_some_and(|id| !files.iter().any(|file| &file.conversation.id == id))
        {
            settings.app.current_conversation_id = None;
            self.write_settings(&settings)?;
        }

        let (current_messages, current_requests) = settings
            .app
            .current_conversation_id
            .as_deref()
            .and_then(|id| files.iter().find(|file| file.conversation.id == id))
            .map(|file| {
                let mut messages = file.messages.clone();
                messages.sort_by(|a, b| (a.created_at, &a.id).cmp(&(b.created_at, &b.id)));
                let mut requests = file.requests.clone();
                requests.sort_by(|a, b| {
                    b.started_at
                        .cmp(&a.started_at)
                        .then_with(|| b.id.cmp(&a.id))
                });
                (messages, requests)
            })
            .unwrap_or_default();

        let mut providers = settings.providers;
        providers.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
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
            conversations,
            current_messages,
            current_requests,
            settings: settings.app,
        })
    }

    pub(super) fn recover_interrupted_locked(&self) -> Result<()> {
        for mut file in self.read_conversations()? {
            let mut changed = false;
            for message in &mut file.messages {
                if matches!(
                    message.status,
                    MessageStatus::Pending | MessageStatus::Streaming
                ) {
                    message.status = MessageStatus::Interrupted;
                    changed = true;
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
