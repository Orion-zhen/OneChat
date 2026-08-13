#[cfg(target_os = "macos")]
use gpui::MouseButton;

#[cfg(target_os = "macos")]
use crate::desktop::pressure_touch::ForceClickChange;

use super::{generation_border, *};

pub(super) fn render_conversation_row(
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

    let selected =
        app.navigation.page == Page::Chat && current_id == Some(conversation.id.as_str());
    let hovered = app.sidebar.hovered_conversation_id.as_deref() == Some(&conversation.id);
    let select_id = conversation.id.clone();
    let hover_id = conversation.id.clone();
    let pin_id = conversation.id.clone();
    let rename_id = conversation.id.clone();
    let delete_id = conversation.id.clone();
    let row_id: SharedString = format!("conversation-{}", conversation.id).into();
    let pinned = conversation.pinned;
    let generating = app.is_conversation_generating(&conversation.id);
    let unseen_generation = app
        .sidebar
        .unseen_generations
        .get(&conversation.id)
        .cloned();
    let title_waiting = conversation.auto_title_state == AutoTitleState::Running
        && !generating
        && unseen_generation.is_none();
    let title_animation_id: SharedString =
        format!("waiting-sidebar-title-{}", conversation.id).into();
    let displayed_title = animated_title.unwrap_or(&conversation.title).to_string();
    let title_accessibility_label = if generating {
        format!("{displayed_title}, generating response")
    } else if unseen_generation.is_some() {
        format!("{displayed_title}, response ready")
    } else {
        displayed_title.clone()
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
        let mut rename_button = icon_button(
            SharedString::from(format!("rename-{}", rename_id)),
            AppIcon::Pencil,
            IconTone::Muted,
            cx,
        );
        #[cfg(target_os = "macos")]
        {
            let pressure_id = rename_id.clone();
            rename_button = rename_button
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, _| {
                        this.sidebar.rename_force_click.cancel();
                        this.sidebar.force_renamed_conversation_id = None;
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _, _, _| {
                        this.sidebar.rename_force_click.cancel();
                        this.sidebar.force_renamed_conversation_id = None;
                    }),
                )
                .on_mouse_pressure(cx.listener(move |this, event, _, cx| {
                    if this.sidebar.rename_force_click.update(event) == ForceClickChange::Triggered
                    {
                        this.sidebar.force_renamed_conversation_id = Some(pressure_id.clone());
                        this.regenerate_auto_title(pressure_id.clone(), cx);
                        cx.stop_propagation();
                    }
                }));
        }
        let rename_button = rename_button.on_click(cx.listener(
            move |this, event: &gpui::ClickEvent, window, cx| {
                #[cfg(target_os = "macos")]
                if this.sidebar.force_renamed_conversation_id.as_deref() == Some(rename_id.as_str())
                {
                    this.sidebar.force_renamed_conversation_id = None;
                    return;
                }
                if event.modifiers().secondary() {
                    this.regenerate_auto_title(rename_id.clone(), cx);
                } else {
                    this.start_rename(rename_id.clone(), window, cx);
                }
            },
        ));
        actions = actions.child(rename_button).child(
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

    let mut title_content = div()
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
        .text_base()
        .aria_label(title_accessibility_label)
        .font_weight(if selected {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        });
    #[cfg(target_os = "macos")]
    {
        let pressure_id = select_id.clone();
        title_content = title_content
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.begin_conversation_peek_pressure(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.cancel_conversation_peek_pressure(cx)),
            )
            .on_mouse_pressure(cx.listener(
                move |this, event: &gpui::MousePressureEvent, _, cx| {
                    if this.update_conversation_peek_pressure(
                        pressure_id.clone(),
                        f32::from(event.position.y),
                        event,
                        cx,
                    ) {
                        cx.stop_propagation();
                    }
                },
            ));
    }
    let title = waiting_title(
        title_content
            .on_click(cx.listener(move |this, _, _, cx| {
                #[cfg(target_os = "macos")]
                if this.consume_conversation_peek_click(&select_id, cx) {
                    return;
                }
                this.select_conversation(select_id.clone(), cx);
            }))
            .child(displayed_title),
        title_animation_id,
        title_waiting,
    );

    let generation_border = if generating {
        Some(generation_border::render_generating(
            conversation.id.as_str(),
            app.sidebar.generation_border_clock(&conversation.id),
            cx.theme().primary,
            cx.reduce_motion(),
        ))
    } else {
        unseen_generation.map(|notice| {
            generation_border::render_completed(
                conversation.id.as_str(),
                notice.request_id.as_str(),
                notice.completion_phase,
                cx.theme().primary,
                cx.reduce_motion(),
            )
        })
    };
    let selected_background = cx.theme().sidebar_accent;
    let hover_background = cx.theme().list_hover;
    div()
        .id(row_id)
        .relative()
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
        .children(generation_border)
        .into_any_element()
}
