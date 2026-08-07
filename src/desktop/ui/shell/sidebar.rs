use super::*;

pub(super) const COLLAPSED_SIDEBAR_WIDTH: f32 = 68.0;
pub(super) const EXPANDED_SIDEBAR_WIDTH: f32 = 260.0;

pub(super) fn render_sidebar(
    app: &mut OneChat,
    animated_title: Option<&str>,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    if app.settings().sidebar_collapsed {
        let sidebar = div()
            .w(px(COLLAPSED_SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .border_r_1()
            .border_color(colors.border)
            .bg(colors.sidebar)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .pt_3()
                    .child(
                        large_icon_button(
                            "expand-sidebar",
                            Icon::Menu,
                            IconTone::Muted,
                            colors,
                            scale_factor,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                    )
                    .child(
                        primary_icon_button(
                            "new-conversation-collapsed",
                            Icon::Plus,
                            colors,
                            scale_factor,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.create_conversation(cx))),
                    ),
            )
            .child(div().flex_1())
            .child(
                large_icon_button(
                    "settings-collapsed",
                    Icon::Settings,
                    IconTone::Muted,
                    colors,
                    scale_factor,
                )
                .mb_3()
                .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Settings, cx))),
            );
        return animate_sidebar(sidebar, true);
    }

    let groups = app.conversation_groups();
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
        .px_2()
        .pb_3();
    if groups.is_empty() {
        list = list.child(
            div()
                .px_3()
                .py_5()
                .text_sm()
                .text_color(colors.muted)
                .child(if app.sidebar.search_query.trim().is_empty() {
                    "No conversations yet"
                } else {
                    "No matching conversations"
                }),
        );
    } else {
        for (group, conversations) in groups {
            list = list.child(
                div()
                    .pt_4()
                    .pb_2()
                    .px_2()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.muted)
                    .child(group.label().to_uppercase()),
            );
            for conversation in conversations {
                list = list.child(render_conversation_row(
                    app,
                    conversation,
                    current_id.as_deref(),
                    animated_title,
                    colors,
                    scale_factor,
                    cx,
                ));
            }
        }
    }

    let sidebar = div()
        .w(px(EXPANDED_SIDEBAR_WIDTH))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(colors.border)
        .bg(colors.sidebar)
        .child(
            div()
                .px_3()
                .pt_3()
                .pb_3()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .h(px(32.0))
                        .px_1()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(colors.muted)
                                .child("Conversations"),
                        )
                        .child(
                            large_icon_button(
                                "collapse-sidebar",
                                Icon::ChevronLeft,
                                IconTone::Muted,
                                colors,
                                scale_factor,
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                        ),
                )
                .child(
                    primary_button_base("new-conversation", colors)
                        .w_full()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(render_icon(
                            Icon::Plus,
                            IconTone::OnAccent,
                            colors,
                            scale_factor,
                            16.0,
                        ))
                        .child("New Conversation")
                        .on_click(cx.listener(|this, _, _, cx| this.create_conversation(cx))),
                )
                .child(app.sidebar.search_input.clone()),
        )
        .child(list)
        .child(render_connection_footer(app, colors, scale_factor, cx));
    animate_sidebar(sidebar, false)
}

fn animate_sidebar(sidebar: Div, collapsed: bool) -> AnyElement {
    let id = if collapsed {
        "sidebar-collapsed"
    } else {
        "sidebar-expanded"
    };
    sidebar
        .overflow_hidden()
        .with_animation(
            id,
            Animation::new(Duration::from_millis(220)).with_easing(ease_out_quint()),
            move |sidebar, delta| {
                let width = if collapsed {
                    EXPANDED_SIDEBAR_WIDTH
                        - (EXPANDED_SIDEBAR_WIDTH - COLLAPSED_SIDEBAR_WIDTH) * delta
                } else {
                    COLLAPSED_SIDEBAR_WIDTH
                        + (EXPANDED_SIDEBAR_WIDTH - COLLAPSED_SIDEBAR_WIDTH) * delta
                };
                sidebar.opacity(0.8 + delta * 0.2).w(px(width))
            },
        )
        .into_any_element()
}

fn render_conversation_row(
    app: &OneChat,
    conversation: Conversation,
    current_id: Option<&str>,
    animated_title: Option<&str>,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    if let Some(input) = app.rename_input(&conversation.id) {
        return div()
            .mb_1()
            .rounded_lg()
            .bg(colors.raised)
            .p_1()
            .child(input)
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

    let mut actions = div()
        .w(px(if hovered {
            80.0
        } else if pinned {
            24.0
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
                Icon::Pin,
                if pinned {
                    IconTone::Accent
                } else {
                    IconTone::Muted
                },
                colors,
                scale_factor,
            )
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_pin(pin_id.clone(), cx))),
        );
    }
    if hovered {
        actions = actions
            .child(
                icon_button(
                    SharedString::from(format!("rename-{}", rename_id)),
                    Icon::Pencil,
                    IconTone::Muted,
                    colors,
                    scale_factor,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.start_rename(rename_id.clone(), window, cx)
                })),
            )
            .child(
                icon_button(
                    SharedString::from(format!("delete-{}", delete_id)),
                    Icon::Close,
                    IconTone::Danger,
                    colors,
                    scale_factor,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.request_delete_conversation(delete_id.clone(), cx)
                })),
            );
    }

    let displayed_title = if selected {
        animated_title.unwrap_or(&conversation.title).to_string()
    } else {
        conversation.title.clone()
    };
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

    div()
        .id(row_id)
        .mb_1()
        .h(px(38.0))
        .rounded_lg()
        .bg(if selected {
            colors.accent_soft
        } else {
            rgba(0x00000000)
        })
        .hover(move |style| {
            style.bg(if selected {
                colors.accent_soft
            } else {
                colors.hover
            })
        })
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

fn render_connection_footer(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let enabled = app
        .data
        .snapshot
        .providers
        .iter()
        .filter(|provider| provider.enabled)
        .count();
    div()
        .flex_none()
        .border_t_1()
        .border_color(colors.border)
        .p_3()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Settings"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(colors.muted)
                        .child(format!("{enabled} providers enabled")),
                ),
        )
        .child(
            large_icon_button(
                "open-settings",
                Icon::Settings,
                IconTone::Muted,
                colors,
                scale_factor,
            )
            .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Settings, cx))),
        )
        .into_any_element()
}
