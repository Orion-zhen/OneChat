use std::time::Duration;

use gpui::{Context, ScrollWheelEvent, Window, px};

use super::super::{MessageEditorTarget, OneChat, SystemPromptMode};
use crate::desktop::ui::stream::follow_after_scroll;

impl OneChat {
    pub(crate) fn resolve_pending_search_jump(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.chat.pending_search_target.clone() else {
            return;
        };
        if self.current_conversation_id() != Some(target.conversation_id.as_str()) {
            self.chat.pending_search_target = None;
            return;
        }
        if let Some(response_id) = &target.response_id
            && self
                .data
                .snapshot
                .current_turns
                .iter()
                .find(|turn| turn.id == target.turn_id)
                .is_some_and(|turn| turn.response(response_id).is_some())
        {
            self.chat
                .visible_response_ids
                .insert(target.turn_id.clone(), response_id.clone());
        }

        let has_prompt_setup = self.current_conversation().is_some_and(|conversation| {
            !conversation.system_prompt.trim().is_empty()
                || !conversation.assistant_opening.trim().is_empty()
        }) || self.chat.system_prompt_mode == SystemPromptMode::Editing;
        let mut item = usize::from(has_prompt_setup);
        let mut target_item = None;
        for turn in self.current_turns() {
            if turn.id == target.turn_id {
                target_item = Some(item + usize::from(target.response_id.is_some()));
                break;
            }
            item += 1 + usize::from(self.visible_response(turn).is_some());
        }
        let Some(item) = target_item else {
            window.request_animation_frame();
            return;
        };
        if self.chat.message_scroll.bounds_for_item(item).is_none() {
            window.request_animation_frame();
            return;
        }

        self.chat.pending_search_target = None;
        let highlight_id = target
            .response_id
            .clone()
            .unwrap_or_else(|| target.turn_id.clone());
        self.chat.search_highlight_id = Some(highlight_id.clone());
        self.jump_to_timeline_item(item, cx);
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1_200))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.chat.search_highlight_id.as_deref() == Some(highlight_id.as_str()) {
                    this.chat.search_highlight_id = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn message_editor_item(&self) -> Option<usize> {
        let target = &self.chat.message_editor.as_ref()?.target;
        let has_prompt_setup = self.current_conversation().is_some_and(|conversation| {
            !conversation.system_prompt.trim().is_empty()
                || !conversation.assistant_opening.trim().is_empty()
        }) || self.chat.system_prompt_mode == SystemPromptMode::Editing;
        let mut item = usize::from(has_prompt_setup);

        for turn in self.current_turns() {
            if matches!(target, MessageEditorTarget::User(id) if id == &turn.id) {
                return Some(item);
            }
            item += 1;

            if let Some(response) = self.visible_response(turn) {
                if matches!(target, MessageEditorTarget::Assistant(id) if id == &response.id) {
                    return Some(item);
                }
                item += 1;
            }
        }
        None
    }

    pub(crate) fn jump_to_message_editor(&mut self, cx: &mut Context<Self>) {
        if let Some(item) = self.message_editor_item() {
            self.jump_to_timeline_item(item, cx);
        }
    }

    pub(crate) fn on_message_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.chat.message_scroll_motion.cancel();
        let delta = event.delta.pixel_delta(window.line_height()).y;
        let distance =
            self.chat.message_scroll.max_offset().y + self.chat.message_scroll.offset().y;
        self.chat.follow_latest = follow_after_scroll(
            self.chat.follow_latest,
            f32::from(delta),
            f32::from(distance),
        );
        cx.notify();
    }

    pub(crate) fn on_timeline_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.chat.message_scroll_motion.cancel();
        let delta = event.delta.pixel_delta(window.line_height()).y;
        let mut offset = self.chat.message_scroll.offset();
        offset.y = (offset.y + delta).clamp(-self.chat.message_scroll.max_offset().y, px(0.0));
        self.chat.message_scroll.set_offset(offset);
        let distance = self.chat.message_scroll.max_offset().y + offset.y;
        self.chat.follow_latest = follow_after_scroll(
            self.chat.follow_latest,
            f32::from(delta),
            f32::from(distance),
        );
        cx.notify();
        cx.stop_propagation();
    }

    pub(crate) fn jump_to_latest(&mut self, cx: &mut Context<Self>) {
        let from = f32::from(self.chat.message_scroll.offset().y);
        let target = -f32::from(self.chat.message_scroll.max_offset().y);
        if (target - from).abs() < 1.0 {
            self.chat.follow_latest = true;
            self.chat.message_scroll.scroll_to_bottom();
        } else {
            self.chat.message_scroll_motion.start(from, target, true);
        }
        cx.notify();
    }

    pub(crate) fn set_timeline_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if self.chat.timeline.hovered == hovered {
            return;
        }
        self.chat.timeline.hovered = hovered;
        if !hovered {
            self.chat.timeline.pointer_y = None;
        }
        self.chat.timeline.expansion_motion.set_visible(hovered);
        cx.notify();
    }

    pub(crate) fn update_timeline_pointer(
        &mut self,
        pointer_y: f32,
        active_item: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        self.chat.timeline.pointer_y = Some(pointer_y);
        self.chat.timeline.active_item = active_item;
        cx.notify();
    }

    pub(crate) fn move_timeline_selection(
        &mut self,
        items: &[usize],
        direction: isize,
        cx: &mut Context<Self>,
    ) {
        if items.is_empty() {
            return;
        }
        let next = if let Some(current) = self
            .chat
            .timeline
            .active_item
            .and_then(|active| items.iter().position(|item| *item == active))
        {
            (current as isize + direction).clamp(0, items.len() as isize - 1) as usize
        } else {
            let top = self.chat.message_scroll.top_item();
            items
                .iter()
                .position(|item| *item >= top)
                .unwrap_or(items.len() - 1)
        };
        self.chat.timeline.active_item = Some(items[next]);
        self.chat.timeline.pointer_y = None;
        cx.notify();
    }

    pub(crate) fn jump_to_timeline_item(&mut self, item: usize, cx: &mut Context<Self>) {
        let Some(bounds) = self.chat.message_scroll.bounds_for_item(item) else {
            return;
        };
        let viewport = self.chat.message_scroll.bounds();
        let inset = 20.0;
        let from = f32::from(self.chat.message_scroll.offset().y);
        let max_offset = f32::from(self.chat.message_scroll.max_offset().y);
        let target =
            (f32::from(viewport.top()) + inset - f32::from(bounds.top())).clamp(-max_offset, 0.0);
        let settle_at_bottom = self.chat.message_scroll.bounds_for_item(item + 1).is_none()
            && (target + max_offset).abs() < 1.0;
        self.chat.follow_latest = false;
        self.chat.timeline.active_item = Some(item);
        if (target - from).abs() < 1.0 || cx.reduce_motion() {
            self.chat.message_scroll_motion.cancel();
            let mut offset = self.chat.message_scroll.offset();
            offset.y = gpui::px(target);
            self.chat.message_scroll.set_offset(offset);
            if settle_at_bottom {
                self.chat.follow_latest = true;
                self.chat.message_scroll.scroll_to_bottom();
            }
        } else {
            self.chat
                .message_scroll_motion
                .start(from, target, settle_at_bottom);
        }
        cx.notify();
    }

    pub(crate) fn advance_message_scroll(&mut self, window: &mut Window) {
        let Some((offset_y, finished, settle_at_bottom)) =
            self.chat.message_scroll_motion.offset(window)
        else {
            return;
        };

        let mut offset = self.chat.message_scroll.offset();
        offset.y = gpui::px(offset_y);
        self.chat.message_scroll.set_offset(offset);
        if finished && settle_at_bottom {
            self.chat.follow_latest = true;
            self.chat.message_scroll.scroll_to_bottom();
        }
    }
}
