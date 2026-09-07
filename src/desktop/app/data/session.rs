use gpui::{AppContext as _, Context, Task};

use super::super::{OneChat, TitleTransition};
use crate::{
    domain::{AutoTitleState, Conversation, ConversationSession},
    storage::{Storage, StorageResult},
};

impl OneChat {
    pub(in crate::desktop::app) fn spawn_storage<T, F>(
        &mut self,
        operation: F,
        complete: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) where
        T: Send + 'static,
        F: FnOnce(&Storage) -> StorageResult<T> + Send + 'static,
    {
        let previous = std::mem::replace(&mut self.data.storage_task, Task::ready(()));
        let storage = self.services.storage.clone();
        self.data.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move { operation(&storage) })
                .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(value) => complete(this, value, cx),
                    Err(error) => {
                        this.chat.pending_search_target = None;
                        this.data.error = Some(format!("Storage error: {error}"));
                    }
                }
                cx.notify();
            });
        });
    }

    pub(in crate::desktop::app) fn edit_current_session(
        &mut self,
        edit: impl FnOnce(&mut ConversationSession) -> Result<(), String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.current_conversation_id().map(str::to_string) else {
            return;
        };
        if self.is_transient_conversation(&id) {
            let Some(session) = self.data.snapshot.current.as_mut() else {
                return;
            };
            match edit(session) {
                Ok(()) => {
                    let conversation = session.conversation.clone();
                    self.data
                        .snapshot
                        .conversation_search
                        .insert_conversation(id, &session.turns);
                    self.data.snapshot.update_conversation_summary(conversation);
                    self.refresh_conversation_content(cx);
                }
                Err(error) => self.data.error = Some(error),
            }
            cx.notify();
        } else {
            self.spawn_storage(
                move |storage| storage.update_session(&id, edit),
                Self::apply_conversation_session,
                cx,
            );
        }
    }

    pub(in crate::desktop::app) fn apply_conversation_metadata(
        &mut self,
        conversation: Conversation,
        cx: &mut Context<Self>,
    ) {
        let visible = self.current_conversation_id() == Some(conversation.id.as_str());
        self.update_title_transition(&conversation);
        self.data.snapshot.update_conversation_summary(conversation);
        if visible {
            self.chat.controls_dirty = true;
            self.refresh_markdown_documents(cx);
        }
    }

    pub(in crate::desktop::app) fn update_title_transition(&mut self, conversation: &Conversation) {
        if conversation.auto_title_state == AutoTitleState::Finished
            && let Some(pending) = self.chat.pending_title_transitions.remove(&conversation.id)
            && conversation.title == pending.new_title
        {
            self.chat.title_transitions.insert(
                conversation.id.clone(),
                TitleTransition::new(&pending.old_title, &pending.new_title),
            );
        }
        if self
            .chat
            .title_transitions
            .get(&conversation.id)
            .is_some_and(|transition| transition.new_title != conversation.title)
        {
            self.chat.title_transitions.remove(&conversation.id);
        }
    }

    pub(in crate::desktop::app) fn apply_conversation_session(
        &mut self,
        session: ConversationSession,
        cx: &mut Context<Self>,
    ) {
        let selected_id = self.current_conversation_id().map(str::to_string);
        let visible = selected_id.as_deref() == Some(session.conversation.id.as_str());
        self.update_title_transition(&session.conversation);
        self.data
            .snapshot
            .apply_conversation(session, selected_id.as_deref());
        self.data.error = None;
        if visible {
            self.refresh_conversation_content(cx);
        }
    }
}
