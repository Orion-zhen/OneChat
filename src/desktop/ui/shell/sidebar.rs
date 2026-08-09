use super::*;

pub(super) fn render_sidebar(
    app: &mut OneChat,
    animated_title: Option<&str>,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let groups = app.conversation_groups(cx);
    let current_id = app
        .settings()
        .current_conversation_id
        .as_deref()
        .map(str::to_owned);
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
        .w(px(SIDEBAR_WIDTH))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().sidebar)
        .child(render_sidebar_header(app, cx))
        .child(list)
        .child(render_sidebar_footer(cx))
        .into_any_element()
}

fn render_sidebar_header(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
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
                        .child(
                            icon_button("new-conversation", AppIcon::Compose, IconTone::Muted, cx)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.create_conversation(cx)),
                                ),
                        )
                        .child(
                            icon_button("collapse-sidebar", AppIcon::Sidebar, IconTone::Muted, cx)
                                .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                        ),
                ),
        )
        .child(
            Input::new(&app.sidebar.search_input)
                .prefix(render_icon(AppIcon::Search, IconTone::Muted, 14.0, cx))
                .cleanable(true)
                .aria_label("Search conversations"),
        )
        .into_any_element()
}

fn render_sidebar_footer(cx: &mut Context<OneChat>) -> AnyElement {
    div()
        .flex_none()
        .p_2()
        .child(
            button_base("open-settings")
                .ghost()
                .w_full()
                .h(px(34.0))
                .px_2()
                .rounded(px(7.0))
                .tooltip("Open settings")
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
                )
                .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Settings, cx))),
        )
        .into_any_element()
}

fn render_conversation_row(
    app: &OneChat,
    conversation: Conversation,
    current_id: Option<&str>,
    animated_title: Option<&str>,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    if let Some(input) = app.rename_input(&conversation.id) {
        return div()
            .mb_1()
            .rounded_lg()
            .bg(cx.theme().muted)
            .p_1()
            .on_action(cx.listener(|this, _: &InputEscape, _, cx| this.cancel_rename(cx)))
            .child(Input::new(&input).aria_label("Rename conversation"))
            .into_any_element();
    }

    let selected = current_id == Some(conversation.id.as_str());
    let hovered = app.sidebar.hovered_conversation_id.as_deref() == Some(&conversation.id);
    let select_id = conversation.id.clone();
    let hover_id = conversation.id.clone();
    let pin_id = conversation.id.clone();
    let rename_id = conversation.id.clone();
    let delete_id = conversation.id.clone();
    let row_id: SharedString = format!("conversation-{}", conversation.id).into();
    let pinned = conversation.pinned;
    let title_waiting = selected && conversation.auto_title_state == AutoTitleState::Running;
    let title_animation_id: SharedString =
        format!("waiting-sidebar-title-{}", conversation.id).into();
    let displayed_title = if selected {
        animated_title.unwrap_or(&conversation.title).to_string()
    } else {
        conversation.title.clone()
    };

    let mut actions = div()
        .w(px(if hovered {
            92.0
        } else if pinned {
            28.0
        } else {
            0.0
        }))
        .flex_none()
        .overflow_hidden()
        .flex()
        .items_center()
        .gap_1();
    if pinned || hovered {
        actions = actions.child(
            icon_button(
                SharedString::from(format!("pin-{}", pin_id)),
                AppIcon::Pin,
                if pinned {
                    IconTone::Accent
                } else {
                    IconTone::Muted
                },
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_pin(pin_id.clone(), cx))),
        );
    }
    if hovered {
        actions = actions
            .child(
                icon_button(
                    SharedString::from(format!("rename-{}", rename_id)),
                    AppIcon::Pencil,
                    IconTone::Muted,
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.start_rename(rename_id.clone(), window, cx)
                })),
            )
            .child(
                icon_button(
                    SharedString::from(format!("delete-{}", delete_id)),
                    AppIcon::Trash,
                    IconTone::Danger,
                    cx,
                )
                .on_click(cx.listener(
                    move |this, event: &gpui::ClickEvent, window, cx| {
                        if event.modifiers().secondary() {
                            this.delete_conversation(delete_id.clone(), cx);
                        } else {
                            this.request_delete_conversation(delete_id.clone(), window, cx);
                        }
                    },
                )),
            );
    }

    let title = waiting_title(
        div()
            .id(SharedString::from(format!("select-{}", conversation.id)))
            .min_w_0()
            .flex_1()
            .h_full()
            .flex()
            .items_center()
            .cursor_pointer()
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .text_sm()
            .font_weight(if selected {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            })
            .on_click(
                cx.listener(move |this, _, _, cx| this.select_conversation(select_id.clone(), cx)),
            )
            .child(displayed_title),
        title_animation_id,
        title_waiting,
    );

    let selected_background = cx.theme().sidebar_accent;
    let hover_background = cx.theme().list_hover;
    div()
        .id(row_id)
        .mb_1()
        .h(px(40.0))
        .rounded(px(10.0))
        .bg(if selected {
            selected_background
        } else {
            cx.theme().transparent
        })
        .hover(move |style| {
            style.bg(if selected {
                selected_background
            } else {
                hover_background
            })
        })
        .active(move |style| style.bg(selected_background))
        .on_hover(cx.listener(move |this, hovering, _, cx| {
            let changed = if *hovering {
                if this.sidebar.hovered_conversation_id.as_deref() == Some(hover_id.as_str()) {
                    false
                } else {
                    this.sidebar.hovered_conversation_id = Some(hover_id.clone());
                    true
                }
            } else if this.sidebar.hovered_conversation_id.as_deref() == Some(hover_id.as_str()) {
                this.sidebar.hovered_conversation_id = None;
                true
            } else {
                false
            };
            if changed {
                cx.notify();
            }
        }))
        .flex()
        .items_center()
        .px_2()
        .child(title)
        .child(actions)
        .into_any_element()
}
