mod compose;
mod runtime;
mod title;

use super::OneChat;
use crate::{
    application::generation::ContextPolicy,
    domain::{Conversation, Model, UserMessage},
    storage::{Storage, StorageError},
};

impl OneChat {
    pub(super) fn prepare_with_storage_context<T>(
        &self,
        conversation: &Conversation,
        model: &Model,
        prepare: impl FnOnce(ContextPolicy<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let history_limit = self.effective_history_limit(conversation);
        let include_document_images = model.capabilities.vision;
        let user_message = |user: &UserMessage| {
            if conversation.temporary {
                Storage::message_for_user_with(user, include_document_images, |file| {
                    self.chat
                        .temporary_attachment_files
                        .get(&file.path)
                        .cloned()
                        .ok_or_else(|| {
                            StorageError::InvalidData(format!(
                                "temporary attachment is unavailable: {}",
                                file.name
                            ))
                        })
                })
            } else {
                self.services.storage.message_for_user(
                    &conversation.id,
                    user,
                    include_document_images,
                )
            }
            .map_err(|error| error.to_string())
        };
        prepare(ContextPolicy::new(history_limit, &user_message))
    }
}
