use chrono::{Local, TimeZone as _};
use gpui::{KeyDownEvent, MouseMoveEvent, Role};

use super::*;

const MARKER_SPACING: f32 = 20.0;
const TRACK_VERTICAL_MARGIN: f32 = 24.0;
const INTERACTION_WIDTH: f32 = 44.0;
const INFLUENCE_RADIUS: f32 = 48.0;
const MAX_HIT_DISTANCE: f32 = 20.0;
const XRAY_WIDTH: f32 = 304.0;
const XRAY_HEIGHT: f32 = 136.0;
const XRAY_MARGIN: f32 = 12.0;
const XRAY_SUMMARY_CHARACTERS: usize = 180;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum TimelineXray {
    User {
        summary: String,
        attachment_count: usize,
    },
    Assistant {
        model: String,
        summary: String,
        status: MessageStatus,
    },
}

impl TimelineXray {
    pub(super) fn user(content: &str, attachment_count: usize) -> Self {
        Self::User {
            summary: text_summary(content, XRAY_SUMMARY_CHARACTERS, Some("No message text")),
            attachment_count,
        }
    }

    pub(super) fn assistant(model: &str, content: &str, status: MessageStatus) -> Self {
        Self::Assistant {
            model: model.to_string(),
            summary: text_summary(
                content,
                XRAY_SUMMARY_CHARACTERS,
                Some(empty_assistant_summary(status)),
            ),
            status,
        }
    }
}

#[derive(Clone)]
pub(super) struct TimelineEntry {
    pub(super) item: usize,
    pub(super) label: String,
    pub(super) timestamp: i64,
    pub(super) xray: TimelineXray,
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
    let xray_marker = if app.chat.timeline.hovered && app.chat.timeline.pointer_y.is_some() {
        active_marker
    } else {
        None
    };

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
        && xray_marker.is_none()
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

    if let Some(marker) = xray_marker {
        timeline = timeline
            .child(
                div()
                    .absolute()
                    .right(px(INTERACTION_WIDTH))
                    .top(px(marker.y - 0.5))
                    .w(px(8.0))
                    .h(px(1.0))
                    .bg(crate::desktop::ui::theme::palette(cx).floating_border),
            )
            .child(render_xray(marker, viewport_height, cx));
    }

    Some(timeline.into_any_element())
}

fn render_xray(marker: &Marker, viewport_height: f32, cx: &App) -> AnyElement {
    let palette = crate::desktop::ui::theme::palette(cx);
    let height = xray_height(viewport_height, XRAY_HEIGHT, XRAY_MARGIN);
    let top = xray_top(marker.y, viewport_height, height, XRAY_MARGIN);
    let timestamp = format_timestamp(marker.entry.timestamp);
    let (title, summary) = match &marker.entry.xray {
        TimelineXray::User { summary, .. } => ("You".to_string(), summary),
        TimelineXray::Assistant { model, summary, .. } => (model.clone(), summary),
    };
    let footer = match &marker.entry.xray {
        TimelineXray::User {
            attachment_count, ..
        } => div()
            .text_size(px(11.0))
            .line_height(px(16.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(cx.theme().muted_foreground)
            .child(match attachment_count {
                1 => "1 attachment".to_string(),
                count => format!("{count} attachments"),
            })
            .into_any_element(),
        TimelineXray::Assistant { status, .. } => status_badge(*status, cx),
    };
    let shadow = vec![BoxShadow {
        color: palette.floating_shadow,
        offset: point(px(0.0), px(10.0)),
        blur_radius: px(28.0),
        spread_radius: px(-9.0),
        inset: false,
    }];

    div()
        .id(("timeline-xray", marker.entry.item))
        .absolute()
        .right(px(INTERACTION_WIDTH + 8.0))
        .top(px(top))
        .w(px(XRAY_WIDTH))
        .h(px(height))
        .overflow_hidden()
        .p_3()
        .rounded(px(14.0))
        .border_1()
        .border_color(palette.floating_border)
        .bg(palette.floating_glass)
        .shadow(shadow)
        .flex()
        .flex_col()
        .gap_2()
        .cursor_default()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.0))
                        .line_height(px(17.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(title),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.0))
                        .line_height(px(15.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(cx.theme().muted_foreground)
                        .child(timestamp),
                ),
        )
        .child(
            div()
                .min_h_0()
                .flex_1()
                .overflow_hidden()
                .line_clamp(3)
                .text_size(px(13.0))
                .line_height(px(18.0))
                .text_color(cx.theme().foreground)
                .child(summary.clone()),
        )
        .child(footer)
        .into_any_element()
}

fn status_badge(status: MessageStatus, cx: &App) -> AnyElement {
    let palette = crate::desktop::ui::theme::palette(cx);
    let (label, background, foreground) = match status {
        MessageStatus::Pending => ("Pending", cx.theme().muted, cx.theme().muted_foreground),
        MessageStatus::Streaming => ("Writing", palette.accent_soft, palette.accent),
        MessageStatus::Completed => ("Completed", cx.theme().muted, cx.theme().muted_foreground),
        MessageStatus::Stopped => ("Stopped", cx.theme().muted, cx.theme().muted_foreground),
        MessageStatus::Failed => ("Failed", palette.danger_soft, palette.danger),
        MessageStatus::Interrupted => ("Interrupted", palette.danger_soft, palette.danger),
    };
    div()
        .self_start()
        .rounded_full()
        .bg(background)
        .px_2()
        .py(px(2.0))
        .text_size(px(10.0))
        .line_height(px(14.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(foreground)
        .child(label)
        .into_any_element()
}

fn empty_assistant_summary(status: MessageStatus) -> &'static str {
    match status {
        MessageStatus::Pending => "Waiting to start…",
        MessageStatus::Streaming => "Waiting for response text…",
        MessageStatus::Completed => "No response text",
        MessageStatus::Stopped => "Stopped before producing text",
        MessageStatus::Failed => "Failed before producing text",
        MessageStatus::Interrupted => "Interrupted before producing text",
    }
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

fn xray_height(viewport_height: f32, preferred_height: f32, margin: f32) -> f32 {
    preferred_height.min((viewport_height - margin * 2.0).max(0.0))
}

fn xray_top(marker_y: f32, viewport_height: f32, card_height: f32, margin: f32) -> f32 {
    let max_top = (viewport_height - card_height - margin).max(margin);
    (marker_y - card_height / 2.0).clamp(margin, max_top)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_xray_summarizes_text_and_attachments() {
        let xray = TimelineXray::user("  A message\nwith   compact whitespace  ", 2);

        assert_eq!(
            xray,
            TimelineXray::User {
                summary: "A message with compact whitespace".into(),
                attachment_count: 2,
            }
        );
    }

    #[test]
    fn user_xray_has_an_understandable_empty_summary() {
        assert_eq!(
            TimelineXray::user(" \n ", 1),
            TimelineXray::User {
                summary: "No message text".into(),
                attachment_count: 1,
            }
        );
    }

    #[test]
    fn assistant_xray_includes_model_status_and_status_aware_empty_summary() {
        assert_eq!(
            TimelineXray::assistant("Sonnet", "", MessageStatus::Streaming),
            TimelineXray::Assistant {
                model: "Sonnet".into(),
                summary: "Waiting for response text…".into(),
                status: MessageStatus::Streaming,
            }
        );
        assert_eq!(
            empty_assistant_summary(MessageStatus::Failed),
            "Failed before producing text"
        );
    }

    #[test]
    fn nearest_marker_uses_the_shared_hit_distance() {
        let markers = [(3, 20.0), (7, 60.0)];

        assert_eq!(nearest_marker(&markers, 42.0, 20.0), Some(7));
        assert_eq!(nearest_marker(&markers, 39.0, 20.0), Some(3));
        assert_eq!(nearest_marker(&markers, 100.0, 20.0), None);
    }

    #[test]
    fn xray_position_is_centered_until_it_reaches_an_edge() {
        assert_eq!(xray_top(200.0, 500.0, 136.0, 12.0), 132.0);
        assert_eq!(xray_top(20.0, 500.0, 136.0, 12.0), 12.0);
        assert_eq!(xray_top(490.0, 500.0, 136.0, 12.0), 352.0);
    }

    #[test]
    fn xray_height_shrinks_to_stay_inside_a_short_viewport() {
        assert_eq!(xray_height(500.0, 136.0, 12.0), 136.0);
        assert_eq!(xray_height(100.0, 136.0, 12.0), 76.0);
    }
}
