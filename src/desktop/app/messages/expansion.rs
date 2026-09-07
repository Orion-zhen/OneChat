use gpui::Context;

use super::super::OneChat;

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
}
