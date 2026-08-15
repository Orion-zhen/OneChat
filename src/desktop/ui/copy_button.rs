use std::time::Duration;

use gpui::{
    App, ClipboardItem, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*,
    px,
};
use gpui_component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
};

use super::icons::{AppIcon, IconTone, render_icon};

const FEEDBACK_DURATION: Duration = Duration::from_millis(1500);
const FEEDBACK_WIDTH: f32 = 60.0;

#[derive(IntoElement)]
pub(crate) struct CopyButton {
    id: ElementId,
    value: SharedString,
    size: f32,
    icon_size: f32,
}

impl CopyButton {
    pub(crate) fn new(id: impl Into<ElementId>, value: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            value: value.into(),
            size: 28.0,
            icon_size: 17.0,
        }
    }
}

#[derive(Default)]
struct CopyFeedbackState {
    revision: u64,
    visible: bool,
}

impl RenderOnce for CopyButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state =
            window.use_keyed_state(self.id.clone(), cx, |_, _| CopyFeedbackState::default());
        let feedback_visible = state.read(cx).visible;
        let value = self.value;
        let button_size = self.size;

        div()
            .relative()
            .size(px(button_size))
            .child(
                Button::new(self.id)
                    .ghost()
                    .tooltip("Copy")
                    .size(px(button_size))
                    .p_0()
                    .child(render_icon(
                        AppIcon::Copy,
                        IconTone::Muted,
                        self.icon_size,
                        cx,
                    ))
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(value.to_string()));

                        let revision = state.update(cx, |state, cx| {
                            state.revision = state.revision.wrapping_add(1);
                            state.visible = true;
                            cx.notify();
                            state.revision
                        });
                        let state = state.clone();
                        cx.spawn(async move |cx| {
                            cx.background_executor().timer(FEEDBACK_DURATION).await;
                            state.update(cx, |state, cx| {
                                if state.revision == revision {
                                    state.visible = false;
                                    cx.notify();
                                }
                            });
                        })
                        .detach();
                    }),
            )
            .when(feedback_visible, |this| {
                this.child(
                    div()
                        .absolute()
                        .bottom(px(button_size + 6.0))
                        .left(px((button_size - FEEDBACK_WIDTH) / 2.0))
                        .w(px(FEEDBACK_WIDTH))
                        .h(px(24.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().popover)
                        .shadow_md()
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .text_color(cx.theme().popover_foreground)
                        .child("Copied"),
                )
            })
    }
}
