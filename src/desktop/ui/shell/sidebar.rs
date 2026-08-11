use std::{f32::consts::PI, time::Duration};

use gpui::{
    Animation, AnimationExt as _, Bounds, Hsla, PathBuilder, Pixels, Window, canvas, ease_in_out,
    ease_out_quint, point,
};

use crate::desktop::app::GenerationBorderClock;

use super::*;

const GENERATION_BORDER_CYCLE: Duration = Duration::from_millis(1_800);
const GENERATION_BORDER_ENTER: Duration = Duration::from_millis(180);
const GENERATION_BORDER_COMPLETE: Duration = Duration::from_millis(240);
const GENERATION_BORDER_SETTLE: Duration = Duration::from_millis(200);
const GENERATION_BEAM_COVERAGE: f32 = 0.2;
const GENERATION_BORDER_STROKE: f32 = 2.0;

pub(super) fn render_sidebar(
    app: &mut OneChat,
    width: f32,
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
    let displayed_title = if selected {
        animated_title.unwrap_or(&conversation.title).to_string()
    } else {
        conversation.title.clone()
    };
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
        actions = actions
            .child(
                icon_button(
                    SharedString::from(format!("rename-{}", rename_id)),
                    AppIcon::Pencil,
                    IconTone::Muted,
                    cx,
                )
                .on_click(cx.listener(
                    move |this, event: &gpui::ClickEvent, window, cx| {
                        if event.modifiers().secondary() {
                            this.regenerate_auto_title(rename_id.clone(), cx);
                        } else {
                            this.start_rename(rename_id.clone(), window, cx);
                        }
                    },
                )),
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
            .text_base()
            .aria_label(title_accessibility_label)
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

    let generation_border = if generating {
        Some(render_generating_border(
            conversation.id.as_str(),
            app.sidebar.generation_border_clock(&conversation.id),
            cx.theme().primary,
            cx.reduce_motion(),
        ))
    } else {
        unseen_generation.map(|notice| {
            render_completed_border(
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

fn render_generating_border(
    conversation_id: &str,
    clock: GenerationBorderClock,
    color: Hsla,
    reduce_motion: bool,
) -> AnyElement {
    if reduce_motion {
        return border_layer(BorderAppearance::Full { opacity: 0.55 }, color);
    }

    let animation_id: SharedString = format!("generating-border-{conversation_id}").into();
    div()
        .absolute()
        .inset_0()
        .with_animations(
            animation_id,
            vec![
                Animation::new(GENERATION_BORDER_ENTER).with_easing(ease_out_quint()),
                Animation::new(GENERATION_BORDER_CYCLE).repeat(),
            ],
            move |layer, animation, delta| {
                layer.child(border_canvas(
                    BorderAppearance::Beam {
                        head: clock.phase(),
                        opacity: if animation == 0 { delta } else { 1.0 },
                    },
                    color,
                ))
            },
        )
        .into_any_element()
}

fn render_completed_border(
    conversation_id: &str,
    request_id: &str,
    completion_phase: f32,
    color: Hsla,
    reduce_motion: bool,
) -> AnyElement {
    if reduce_motion {
        return border_layer(BorderAppearance::Full { opacity: 0.3 }, color);
    }

    let animation_id: SharedString =
        format!("completed-border-{conversation_id}-{request_id}").into();
    div()
        .absolute()
        .inset_0()
        .with_animations(
            animation_id,
            vec![
                Animation::new(GENERATION_BORDER_COMPLETE).with_easing(ease_in_out),
                Animation::new(GENERATION_BORDER_SETTLE).with_easing(ease_out_quint()),
            ],
            move |layer, animation, delta| {
                let appearance = if animation == 0 {
                    BorderAppearance::Completing {
                        head: completion_phase,
                        progress: delta,
                    }
                } else {
                    BorderAppearance::Full {
                        opacity: 0.75 - delta * 0.45,
                    }
                };
                layer.child(border_canvas(appearance, color))
            },
        )
        .into_any_element()
}

fn border_layer(appearance: BorderAppearance, color: Hsla) -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .child(border_canvas(appearance, color))
        .into_any_element()
}

#[derive(Clone, Copy)]
enum BorderAppearance {
    Beam { head: f32, opacity: f32 },
    Completing { head: f32, progress: f32 },
    Full { opacity: f32 },
}

fn border_canvas(appearance: BorderAppearance, color: Hsla) -> AnyElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| paint_conversation_border(bounds, appearance, color, window),
    )
    .absolute()
    .inset_0()
    .into_any_element()
}

fn paint_conversation_border(
    bounds: Bounds<Pixels>,
    appearance: BorderAppearance,
    color: Hsla,
    window: &mut Window,
) {
    let perimeter = rounded_rect_perimeter(bounds);
    match appearance {
        BorderAppearance::Beam { head, opacity } => {
            paint_beam(
                bounds,
                head,
                GENERATION_BEAM_COVERAGE,
                opacity,
                color,
                window,
            );
        }
        BorderAppearance::Completing { head, progress } => {
            let coverage = GENERATION_BEAM_COVERAGE + (1.0 - GENERATION_BEAM_COVERAGE) * progress;
            let start = head * perimeter - coverage * perimeter;
            paint_border_segment(
                bounds,
                start,
                coverage * perimeter,
                color.opacity(0.12 + progress * 0.63),
                window,
            );
            paint_beam(bounds, head, coverage, 1.0 - progress, color, window);
        }
        BorderAppearance::Full { opacity } => {
            paint_border_segment(bounds, 0.0, perimeter, color.opacity(opacity), window);
        }
    }
}

fn paint_beam(
    bounds: Bounds<Pixels>,
    head: f32,
    coverage: f32,
    opacity: f32,
    color: Hsla,
    window: &mut Window,
) {
    let perimeter = rounded_rect_perimeter(bounds);
    let length = perimeter * coverage;
    let head = perimeter * head;
    for (fraction, alpha) in [
        (1.0, 0.12),
        (0.72, 0.15),
        (0.46, 0.20),
        (0.25, 0.28),
        (0.10, 0.42),
    ] {
        let segment_length = length * fraction;
        paint_border_segment(
            bounds,
            head - segment_length,
            segment_length,
            color.opacity(alpha * opacity),
            window,
        );
    }
}

fn rounded_rect_perimeter(bounds: Bounds<Pixels>) -> f32 {
    let width = f32::from(bounds.size.width) - GENERATION_BORDER_STROKE;
    let height = f32::from(bounds.size.height) - GENERATION_BORDER_STROKE;
    let radius = (10.0 - GENERATION_BORDER_STROKE / 2.0)
        .min(width / 2.0)
        .min(height / 2.0);
    2.0 * (width + height - 4.0 * radius) + 2.0 * PI * radius
}

fn paint_border_segment(
    bounds: Bounds<Pixels>,
    start: f32,
    length: f32,
    color: Hsla,
    window: &mut Window,
) {
    let perimeter = rounded_rect_perimeter(bounds);
    let length = length.min(perimeter);
    if length <= 0.0 || color.a <= 0.0 {
        return;
    }
    if length >= perimeter - 0.01 {
        paint_border_range(bounds, None, color, window);
        return;
    }

    let start = start.rem_euclid(perimeter);
    let first_length = length.min(perimeter - start);
    paint_border_range(bounds, Some((start, first_length)), color, window);
    let wrapped_length = length - first_length;
    if wrapped_length > 0.01 {
        paint_border_range(bounds, Some((0.0, wrapped_length)), color, window);
    }
}

fn paint_border_range(
    bounds: Bounds<Pixels>,
    range: Option<(f32, f32)>,
    color: Hsla,
    window: &mut Window,
) {
    let inset = GENERATION_BORDER_STROKE / 2.0;
    let left = f32::from(bounds.origin.x) + inset;
    let top = f32::from(bounds.origin.y) + inset;
    let right = f32::from(bounds.origin.x + bounds.size.width) - inset;
    let bottom = f32::from(bounds.origin.y + bounds.size.height) - inset;
    let radius = (10.0 - inset)
        .min((right - left) / 2.0)
        .min((bottom - top) / 2.0);
    let mut path = PathBuilder::stroke(px(GENERATION_BORDER_STROKE));
    if let Some((start, length)) = range {
        let perimeter = rounded_rect_perimeter(bounds);
        path = path.dash_array(&[
            px(0.0),
            px(start),
            px(length),
            px((perimeter - start - length).max(0.0)),
        ]);
    }
    path.move_to(point(px(left + radius), px(top)));
    path.line_to(point(px(right - radius), px(top)));
    path.arc_to(
        point(px(radius), px(radius)),
        px(0.0),
        false,
        true,
        point(px(right), px(top + radius)),
    );
    path.line_to(point(px(right), px(bottom - radius)));
    path.arc_to(
        point(px(radius), px(radius)),
        px(0.0),
        false,
        true,
        point(px(right - radius), px(bottom)),
    );
    path.line_to(point(px(left + radius), px(bottom)));
    path.arc_to(
        point(px(radius), px(radius)),
        px(0.0),
        false,
        true,
        point(px(left), px(bottom - radius)),
    );
    path.line_to(point(px(left), px(top + radius)));
    path.arc_to(
        point(px(radius), px(radius)),
        px(0.0),
        false,
        true,
        point(px(left + radius), px(top)),
    );
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
