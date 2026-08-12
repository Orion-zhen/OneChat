use gpui::{Context, MousePressureEvent};

use super::super::OneChat;
use crate::desktop::pressure_touch::{self, Feedback, ForceClickGestureChange};

impl OneChat {
    pub(crate) fn begin_response_tab_pressure(&mut self) {
        self.chat.response_tab_force_click.begin();
    }

    pub(crate) fn update_response_tab_pressure(
        &mut self,
        turn_id: String,
        response_id: String,
        event: &MousePressureEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        match self
            .chat
            .response_tab_force_click
            .update(event, response_id.clone())
        {
            ForceClickGestureChange::Triggered(response_id) => {
                if !self.response_can_become_context(&turn_id, &response_id) {
                    self.chat.response_tab_force_click.cancel();
                    return false;
                }
                self.show_response(turn_id.clone(), response_id.clone(), cx);
                self.use_response_for_context(turn_id, response_id, cx);
                pressure_touch::feedback(Feedback::SelectionChanged);
                true
            }
            ForceClickGestureChange::Released(_) => true,
            ForceClickGestureChange::None => false,
        }
    }

    pub(crate) fn cancel_response_tab_pressure(&mut self) {
        self.chat.response_tab_force_click.cancel();
    }

    pub(crate) fn consume_response_tab_click(&mut self, response_id: &String) -> bool {
        self.chat
            .response_tab_force_click
            .consume_click(response_id)
    }

    fn response_can_become_context(&self, turn_id: &str, response_id: &str) -> bool {
        if self.is_current_generating() {
            return false;
        }
        self.current_turns()
            .into_iter()
            .find(|turn| turn.id == turn_id)
            .and_then(|turn| {
                (turn.continuation_response_id.as_deref() != Some(response_id))
                    .then(|| turn.response(response_id))
                    .flatten()
            })
            .is_some_and(|response| response.is_usable_as_context())
    }
}
