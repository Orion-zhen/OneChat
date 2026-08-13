use std::{f32::consts::PI, time::Duration};

use gpui::{
    Animation, AnimationExt as _, AnyElement, Bounds, Hsla, IntoElement as _, ParentElement as _,
    PathBuilder, Pixels, SharedString, Styled as _, Window, canvas, div, ease_in_out,
    ease_out_quint, point, px,
};

use crate::desktop::app::GenerationBorderClock;

const GENERATION_BORDER_CYCLE: Duration = Duration::from_millis(1_800);
const GENERATION_BORDER_ENTER: Duration = Duration::from_millis(180);
const GENERATION_BORDER_COMPLETE: Duration = Duration::from_millis(240);
const GENERATION_BORDER_SETTLE: Duration = Duration::from_millis(200);
const GENERATION_BEAM_COVERAGE: f32 = 0.2;
const GENERATION_BORDER_STROKE: f32 = 2.0;

pub(super) fn render_generating(
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

pub(super) fn render_completed(
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
