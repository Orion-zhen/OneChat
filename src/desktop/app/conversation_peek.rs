use gpui::{AppContext as _, Context, MousePressureEvent};

use super::{ConversationPeekContent, ConversationPeekState, OneChat};
use crate::desktop::pressure_touch::ForceClickGestureChange;

impl ConversationPeekState {
    fn open(
        &mut self,
        conversation_id: String,
        anchor_y: f32,
        content: ConversationPeekContent,
    ) -> u64 {
        self.revision = self.revision.wrapping_add(1);
        self.conversation_id = Some(conversation_id);
        self.anchor_y = anchor_y;
        self.content = content;
        self.revision
    }

    fn close(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.conversation_id = None;
        self.content = ConversationPeekContent::Loading;
    }

    fn complete(
        &mut self,
        conversation_id: &str,
        revision: u64,
        content: ConversationPeekContent,
    ) -> bool {
        if self.revision != revision || self.conversation_id.as_deref() != Some(conversation_id) {
            return false;
        }
        self.content = content;
        true
    }
}

impl OneChat {
    pub(crate) fn begin_conversation_peek_pressure(&mut self, cx: &mut Context<Self>) {
        self.sidebar.conversation_peek.force_click.begin();
        if self.sidebar.conversation_peek.conversation_id.is_some() {
            self.close_conversation_peek(cx);
        }
    }

    pub(crate) fn update_conversation_peek_pressure(
        &mut self,
        conversation_id: String,
        anchor_y: f32,
        event: &MousePressureEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        match self
            .sidebar
            .conversation_peek
            .force_click
            .update(event, conversation_id)
        {
            ForceClickGestureChange::Triggered(conversation_id) => {
                self.open_conversation_peek(conversation_id, anchor_y, cx);
                true
            }
            ForceClickGestureChange::Released(_) => {
                self.close_conversation_peek(cx);
                true
            }
            ForceClickGestureChange::None => false,
        }
    }

    pub(crate) fn cancel_conversation_peek_pressure(&mut self, cx: &mut Context<Self>) {
        let active = self.sidebar.conversation_peek.force_click.cancel();
        if active.is_some() || self.sidebar.conversation_peek.conversation_id.is_some() {
            self.close_conversation_peek(cx);
        }
    }

    pub(crate) fn consume_conversation_peek_click(
        &mut self,
        conversation_id: &String,
        cx: &mut Context<Self>,
    ) -> bool {
        let consumed = self
            .sidebar
            .conversation_peek
            .force_click
            .consume_click(conversation_id);
        if consumed {
            self.close_conversation_peek(cx);
        }
        consumed
    }

    pub(crate) fn close_conversation_peek(&mut self, cx: &mut Context<Self>) {
        if self.sidebar.conversation_peek.conversation_id.is_none() {
            return;
        }
        self.sidebar.conversation_peek.close();
        cx.notify();
    }

    fn open_conversation_peek(
        &mut self,
        conversation_id: String,
        anchor_y: f32,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .current_conversation()
            .is_some_and(|conversation| conversation.id == conversation_id);
        let content = if current {
            ConversationPeekContent::Ready(self.data.snapshot.current_turns.clone())
        } else {
            ConversationPeekContent::Loading
        };
        let revision =
            self.sidebar
                .conversation_peek
                .open(conversation_id.clone(), anchor_y, content);
        cx.notify();

        if current {
            return;
        }

        let storage = self.services.storage.clone();
        cx.spawn(async move |this, cx| {
            let load_id = conversation_id.clone();
            let result = cx
                .background_spawn(async move { storage.load_conversation_turns(&load_id) })
                .await;
            let _ = this.update(cx, |this, cx| {
                let content = result.map_or(
                    ConversationPeekContent::Failed,
                    ConversationPeekContent::Ready,
                );
                if this
                    .sidebar
                    .conversation_peek
                    .complete(&conversation_id, revision, content)
                {
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_load_cannot_replace_a_new_peek() {
        let mut state = ConversationPeekState::default();
        let first = state.open("first".into(), 80.0, ConversationPeekContent::Loading);
        state.close();
        state.open("second".into(), 120.0, ConversationPeekContent::Loading);

        assert!(!state.complete("first", first, ConversationPeekContent::Ready(Vec::new())));
        assert_eq!(state.conversation_id.as_deref(), Some("second"));
        assert!(matches!(state.content, ConversationPeekContent::Loading));
    }

    #[test]
    fn current_load_completes_the_matching_peek() {
        let mut state = ConversationPeekState::default();
        let revision = state.open("current".into(), 80.0, ConversationPeekContent::Loading);

        assert!(state.complete(
            "current",
            revision,
            ConversationPeekContent::Ready(Vec::new())
        ));
        assert!(matches!(
            state.content,
            ConversationPeekContent::Ready(ref turns) if turns.is_empty()
        ));
    }
}
