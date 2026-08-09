use gpui::Context;

use super::super::{COLLAPSED_THINKING_HEIGHT, OneChat, ThinkingMotion};

impl OneChat {
    pub(crate) fn error_detail_expanded(&self, response_id: &str) -> bool {
        self.chat.expanded_error_ids.contains(response_id)
    }

    pub(crate) fn toggle_error_detail(&mut self, response_id: String, cx: &mut Context<Self>) {
        if !self.chat.expanded_error_ids.remove(&response_id) {
            self.chat.expanded_error_ids.insert(response_id);
        }
        cx.notify();
    }

    pub(crate) fn tool_execution_expanded(&self, execution_id: &str) -> bool {
        self.chat.expanded_tool_execution_ids.contains(execution_id)
    }

    pub(crate) fn toggle_tool_execution(&mut self, execution_id: String, cx: &mut Context<Self>) {
        if !self.chat.expanded_tool_execution_ids.remove(&execution_id) {
            self.chat.expanded_tool_execution_ids.insert(execution_id);
        }
        cx.notify();
    }

    pub(crate) fn thinking_expanded(&self, response_id: &str, default_expanded: bool) -> bool {
        default_expanded != self.chat.thinking_expansion_overrides.contains(response_id)
    }

    pub(crate) fn toggle_thinking(
        &mut self,
        response_id: String,
        default_expanded: bool,
        cx: &mut Context<Self>,
    ) {
        let expanding = !self.thinking_expanded(&response_id, default_expanded);
        if !self.chat.thinking_expansion_overrides.remove(&response_id) {
            self.chat
                .thinking_expansion_overrides
                .insert(response_id.clone());
        }
        self.capture_thinking_motion(response_id, !expanding);
        cx.notify();
    }

    pub(crate) fn finish_thinking(&mut self, response_id: String) {
        self.chat.thinking_expansion_overrides.remove(&response_id);
        self.capture_thinking_motion(response_id, true);
    }

    fn capture_thinking_motion(&mut self, response_id: String, scroll_to_bottom: bool) {
        let Some(scroll) = self.chat.thinking_scrolls.get(&response_id) else {
            return;
        };
        let from_height = f32::from(scroll.bounds().size.height);
        let measured_height = from_height + f32::from(scroll.max_offset().y);
        self.chat.thinking_motions.insert(
            response_id,
            ThinkingMotion {
                from_height: if from_height > 0.0 {
                    from_height
                } else {
                    COLLAPSED_THINKING_HEIGHT
                },
                full_height: if measured_height > 0.0 {
                    measured_height
                } else {
                    COLLAPSED_THINKING_HEIGHT
                },
            },
        );
        if scroll_to_bottom {
            scroll.scroll_to_bottom();
        }
    }
}
