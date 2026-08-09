use chrono::{Local, TimeZone as _};
use gpui::{KeyDownEvent, MouseMoveEvent, Role};

use super::*;

const MARKER_SPACING: f32 = 20.0;
const TRACK_VERTICAL_MARGIN: f32 = 24.0;
const INTERACTION_WIDTH: f32 = 44.0;
const INFLUENCE_RADIUS: f32 = 48.0;
const MAX_HIT_DISTANCE: f32 = 20.0;

#[derive(Clone)]
pub(super) struct TimelineEntry {
    pub(super) item: usize,
    pub(super) label: String,
    pub(super) timestamp: i64,
}

#[derive(Clone)]
struct Marker {
    entry: TimelineEntry,
    y: f32,
}

pub(super) fn render(
    app: &OneChat,
    entries: Vec<TimelineEntry>,
    expansion: f32,
    focused: bool,
    reduce_motion: bool,
    cx: &mut Context<OneChat>,
) -> Option<AnyElement> {
    let scroll = &app.chat.message_scroll;
    let viewport = scroll.bounds();
    let viewport_height = f32::from(viewport.size.height);
    let max_offset = f32::from(scroll.max_offset().y);
    if viewport_height <= TRACK_VERTICAL_MARGIN * 2.0 || max_offset <= 8.0 || entries.len() < 2 {
        return None;
    }

    let entries = entries
        .into_iter()
        .filter(|entry| scroll.bounds_for_item(entry.item).is_some())
        .collect::<Vec<_>>();
    if entries.len() < 2 {
        return None;
    }
    let entry_count = entries.len();
    let track_height = timeline_height(entry_count, MARKER_SPACING);
    let available_height = viewport_height - TRACK_VERTICAL_MARGIN * 2.0;
    let scroll_progress = (-f32::from(scroll.offset().y) / max_offset).clamp(0.0, 1.0);
    let track_top = if track_height <= available_height {
        (viewport_height - track_height) / 2.0
    } else {
        TRACK_VERTICAL_MARGIN - scroll_progress * (track_height - available_height)
    };
    let markers = entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| Marker {
            y: track_top + evenly_spaced_position(index, MARKER_SPACING),
            entry,
        })
        .collect::<Vec<_>>();

    let transition_progress = if focused { 1.0 } else { expansion };
    let interaction_progress = if app.chat.timeline.hovered || focused {
        1.0
    } else {
        transition_progress
    };
    let active_item = app.chat.timeline.active_item;
    let pointer_y = app.chat.timeline.pointer_y.or_else(|| {
        active_item.and_then(|active| {
            markers
                .iter()
                .find(|marker| marker.entry.item == active)
                .map(|marker| marker.y)
        })
    });
    let marker_targets = markers
        .iter()
        .map(|marker| (marker.entry.item, marker.y))
        .collect::<Vec<_>>();
    let item_indices = markers
        .iter()
        .map(|marker| marker.entry.item)
        .collect::<Vec<_>>();
    let active_marker =
        active_item.and_then(|active| markers.iter().find(|marker| marker.entry.item == active));

    let track_color = cx.theme().muted_foreground;
    let active_color = cx.theme().foreground;
    let border = cx.theme().border;
    let popover = cx.theme().popover;
    let foreground = cx.theme().foreground;
    let muted_foreground = cx.theme().muted_foreground;

    let mut timeline = div()
        .id("message-timeline")
        .absolute()
        .right_0()
        .top_0()
        .bottom_0()
        .w(px(INTERACTION_WIDTH))
        .track_focus(&app.chat.timeline.focus)
        .role(Role::Toolbar)
        .aria_label("Conversation timeline")
        .on_hover(cx.listener(|this, hovering, _, cx| this.set_timeline_hovered(*hovering, cx)))
        .on_scroll_wheel(cx.listener(OneChat::on_timeline_scroll))
        .on_mouse_move({
            let targets = marker_targets.clone();
            cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                let viewport_top = f32::from(this.chat.message_scroll.bounds().top());
                let pointer_y = f32::from(event.position.y) - viewport_top;
                let active = nearest_marker(&targets, pointer_y, MAX_HIT_DISTANCE);
                this.update_timeline_pointer(pointer_y, active, cx);
                cx.stop_propagation();
            })
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|_, _, _, cx| cx.stop_propagation()),
        )
        .on_click(cx.listener(|this, _, _, cx| {
            if let Some(item) = this.chat.timeline.active_item {
                this.jump_to_timeline_item(item, cx);
            }
            cx.stop_propagation();
        }))
        .on_key_down({
            let items = item_indices.clone();
            cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "up" => {
                        this.move_timeline_selection(&items, -1, cx);
                        cx.stop_propagation();
                    }
                    "down" => {
                        this.move_timeline_selection(&items, 1, cx);
                        cx.stop_propagation();
                    }
                    "enter" => {
                        if let Some(item) = this.chat.timeline.active_item {
                            this.jump_to_timeline_item(item, cx);
                        }
                        cx.stop_propagation();
                    }
                    "escape" => {
                        this.chat.timeline.active_item = None;
                        this.chat.timeline.pointer_y = None;
                        window.focus(&this.root_focus, cx);
                        cx.notify();
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            })
        })
        .when(active_item.is_some(), |timeline| timeline.cursor_pointer());

    for marker in &markers {
        let distance = pointer_y.map_or(f32::INFINITY, |pointer| marker.y - pointer);
        let influence = influence(distance.abs(), INFLUENCE_RADIUS) * interaction_progress;
        let active = active_item == Some(marker.entry.item) && interaction_progress > 0.01;
        let scale = if reduce_motion {
            1.0 + 0.25 * influence
        } else {
            1.0 + 0.5 * influence
        };
        let width = 24.0 * scale;
        let height = 4.0 + 1.5 * influence;
        timeline = timeline.child(
            div()
                .absolute()
                .right(px(6.0))
                .top(px(marker.y - height / 2.0))
                .w(px(width))
                .h(px(height))
                .rounded_full()
                .bg(if active { active_color } else { track_color })
                .opacity(if active {
                    0.96
                } else {
                    0.46 + influence * 0.24
                }),
        );
    }

    if interaction_progress > 0.01
        && let Some(marker) = active_marker
    {
        let label = format!(
            "{} · {}",
            marker.entry.label,
            format_timestamp(marker.entry.timestamp)
        );
        timeline = timeline.child(
            div()
                .absolute()
                .right(px(46.0 + 4.0 * (1.0 - transition_progress)))
                .top(px((marker.y - 12.0).clamp(4.0, viewport_height - 28.0)))
                .h(px(24.0))
                .px_2()
                .rounded(px(8.0))
                .border_1()
                .border_color(border)
                .bg(popover)
                .shadow_sm()
                .flex()
                .items_center()
                .whitespace_nowrap()
                .text_size(px(11.0))
                .line_height(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if focused {
                    foreground
                } else {
                    muted_foreground
                })
                .opacity(transition_progress)
                .child(label),
        );
    }

    Some(timeline.into_any_element())
}

fn timeline_height(count: usize, spacing: f32) -> f32 {
    count.saturating_sub(1) as f32 * spacing
}

fn evenly_spaced_position(index: usize, spacing: f32) -> f32 {
    index as f32 * spacing
}

fn influence(distance: f32, radius: f32) -> f32 {
    if radius <= 0.0 || distance >= radius {
        return 0.0;
    }
    let value = 1.0 - distance / radius;
    value * value * (3.0 - 2.0 * value)
}

fn nearest_marker(markers: &[(usize, f32)], pointer_y: f32, max_distance: f32) -> Option<usize> {
    markers
        .iter()
        .min_by(|left, right| {
            (left.1 - pointer_y)
                .abs()
                .total_cmp(&(right.1 - pointer_y).abs())
        })
        .filter(|marker| (marker.1 - pointer_y).abs() <= max_distance)
        .map(|marker| marker.0)
}

fn format_timestamp(timestamp: i64) -> String {
    let Some(date_time) = Local.timestamp_opt(timestamp, 0).single() else {
        return "Unknown time".into();
    };
    if date_time.date_naive() == Local::now().date_naive() {
        date_time.format("%H:%M").to_string()
    } else {
        date_time.format("%b %-d, %H:%M").to_string()
    }
}
