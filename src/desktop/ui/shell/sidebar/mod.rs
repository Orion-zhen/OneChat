mod conversation_row;
mod generation_border;

use std::collections::HashMap;

use super::*;
use conversation_row::render_conversation_row;

#[cfg(target_os = "macos")]
use crate::desktop::pressure_touch::ForceClickChange;

pub(super) fn render_sidebar(
    app: &mut OneChat,
    width: f32,
    animated_titles: &HashMap<String, String>,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let groups = app.conversation_groups(cx);
    let current_id = app.current_conversation_id().map(str::to_owned);
    let mut list = div()
        .id("conversation-list")
        .min_h_0()
        .flex_1()
        .overflow_y_scroll()
        .px_3()
        .pb_3();

    if groups.is_empty() {
        list = list.child(
            div()
                .px_3()
                .py_5()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(
                    if app.sidebar.search_input.read(cx).value().trim().is_empty() {
                        "No conversations yet"
                    } else {
                        "No matching conversations"
                    },
                ),
        );
    } else {
        for (group, conversations) in groups {
            list = list.child(
                div()
                    .pt_4()
                    .pb_2()
                    .px_1()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().muted_foreground)
                    .child(group.label()),
            );
            for conversation in conversations {
                let animated_title = animated_titles.get(&conversation.id).map(String::as_str);
                list = list.child(render_conversation_row(
                    app,
                    conversation,
                    current_id.as_deref(),
                    animated_title,
                    cx,
                ));
            }
        }
    }

    div()
        .w(px(width))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().sidebar)
        .child(render_sidebar_header(app, cx))
        .child(list)
        .child(render_sidebar_footer(app, cx))
        .into_any_element()
}

fn render_sidebar_header(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let new_conversation = icon_button("new-conversation", AppIcon::Compose, IconTone::Muted, cx);
    #[cfg(target_os = "macos")]
    let new_conversation = new_conversation
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, _| {
                this.sidebar.new_conversation_force_click.cancel();
                this.sidebar.force_created_temporary_conversation = false;
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|this, _, _, _| {
                this.sidebar.new_conversation_force_click.cancel();
            }),
        )
        .on_mouse_pressure(cx.listener(|this, event, _, cx| {
            if this.sidebar.new_conversation_force_click.update(event)
                == ForceClickChange::Triggered
            {
                this.sidebar.force_created_temporary_conversation = true;
                this.set_page(Page::Chat, cx);
                this.create_temporary_conversation(cx);
                cx.stop_propagation();
            }
        }));
    let new_conversation =
        new_conversation.on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
            #[cfg(target_os = "macos")]
            if std::mem::take(&mut this.sidebar.force_created_temporary_conversation) {
                return;
            }
            this.set_page(Page::Chat, cx);
            if event.modifiers().secondary() {
                this.create_temporary_conversation(cx);
            } else {
                this.create_conversation(cx);
            }
        }));

    div()
        .px_3()
        .pt_3()
        .pb_2()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .h(px(36.0))
                .pl_1()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(20.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Chats"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(new_conversation)
                        .child(
                            icon_button("collapse-sidebar", AppIcon::Sidebar, IconTone::Muted, cx)
                                .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                        ),
                ),
        )
        .child(
            Input::new(&app.sidebar.search_input)
                .prefix(render_icon(AppIcon::Search, IconTone::Muted, 16.0, cx))
                .cleanable(true)
                .aria_label("Search conversations")
                .h(px(40.0))
                .text_base(),
        )
        .into_any_element()
}

fn render_sidebar_footer(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let tts_selected = app.navigation.page == Page::Tts;
    div()
        .flex_none()
        .p_2()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            button_base("open-text-to-speech")
                .ghost()
                .selected(tts_selected)
                .w_full()
                .h(px(36.0))
                .px_2p5()
                .rounded(px(9.0))
                .tooltip("Open Text to Speech")
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_start()
                        .gap_2()
                        .child(render_icon(
                            AppIcon::AudioLines,
                            if tts_selected {
                                IconTone::Accent
                            } else {
                                IconTone::Muted
                            },
                            16.0,
                            cx,
                        ))
                        .child(
                            div()
                                .font_weight(if tts_selected {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .child("Text to Speech"),
                        ),
                )
                .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Tts, cx))),
        )
        .child(
            button_base("open-settings")
                .ghost()
                .w_full()
                .h(px(36.0))
                .px_2p5()
                .rounded(px(9.0))
                .tooltip("Open settings")
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(render_icon(AppIcon::Settings, IconTone::Muted, 16.0, cx))
                                .child("Settings"),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(shortcut_label(",")),
                        ),
                )
                .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Settings, cx))),
        )
        .into_any_element()
}
