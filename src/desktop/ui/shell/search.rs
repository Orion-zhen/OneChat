use gpui::{AnyElement, Context, Focusable as _, FontWeight, MouseButton, div, prelude::*, px};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    list::List,
};

use crate::desktop::app::OneChat;

pub(crate) fn render_conversation_search_overlay(
    app: &OneChat,
    progress: f32,
    reduce_motion: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let list = app.overlays.conversation_search.clone();
    let focus = list.read(cx).focus_handle(cx);
    let panel = super::floating_overlay::panel(
        "conversation-search-panel",
        "Search conversations",
        &focus,
        760.0,
        14.0,
        cx,
    )
    .gap_2()
    .child(
        div()
            .h(px(40.0))
            .px_2()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(17.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Search conversations"),
            )
            .child(
                Button::new("close-conversation-search")
                    .ghost()
                    .tooltip("Close")
                    .size(px(34.0))
                    .p_0()
                    .rounded(px(11.0))
                    .child(Icon::new(IconName::Close).size(px(18.0)))
                    .on_click(cx.listener(|this, _, _, cx| this.close_shell_overlay(true, cx))),
            ),
    )
    .child(
        List::new(&list)
            .large()
            .search_placeholder("Search titles and messages…")
            .h(px(500.0))
            .min_h_0()
            .flex_1()
            .w_full()
            .rounded(px(14.0))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .overflow_hidden()
            .scrollbar_visible(false),
    )
    .child(super::pickers::picker_help(cx));

    super::floating_overlay::backdrop(
        "conversation-search-overlay",
        panel,
        progress,
        reduce_motion,
        cx,
    )
    .on_mouse_down(
        MouseButton::Left,
        cx.listener(|this, _, _, cx| this.close_shell_overlay(true, cx)),
    )
    .into_any_element()
}
