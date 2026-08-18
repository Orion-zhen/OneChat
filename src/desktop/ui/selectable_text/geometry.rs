use super::*;

#[cfg(target_os = "macos")]
pub(super) fn nearest_index(layout: &gpui::TextLayout, position: gpui::Point<Pixels>) -> usize {
    layout
        .index_for_position(position)
        .unwrap_or_else(|index| index)
        .min(layout.len())
}

fn selection_quad_bounds(
    start: Point<Pixels>,
    end: Point<Pixels>,
    bounds: Bounds<Pixels>,
    line_height: Pixels,
) -> Vec<Bounds<Pixels>> {
    if start.y == end.y {
        return vec![Bounds::from_corners(
            start,
            Point::new(end.x, end.y + line_height),
        )];
    }

    let mut quads = vec![Bounds::from_corners(
        start,
        Point::new(bounds.right(), start.y + line_height),
    )];
    if end.y > start.y + line_height {
        quads.push(Bounds::from_corners(
            Point::new(bounds.left(), start.y + line_height),
            Point::new(bounds.right(), end.y),
        ));
    }
    quads.push(Bounds::from_corners(
        Point::new(bounds.left(), end.y),
        Point::new(end.x, end.y + line_height),
    ));
    quads
}

pub(super) fn selection_quads(
    layout: &gpui::TextLayout,
    selection: &Range<usize>,
    color: Rgba,
) -> Vec<PaintQuad> {
    if selection.is_empty() {
        return Vec::new();
    }
    let (Some(start), Some(end)) = (
        layout.position_for_index(selection.start),
        layout.position_for_index(selection.end),
    ) else {
        return Vec::new();
    };

    selection_quad_bounds(start, end, layout.bounds(), layout.line_height())
        .into_iter()
        .map(|bounds| fill(bounds, color))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px, size};

    #[test]
    fn single_line_selection_uses_one_tight_quad() {
        let bounds = Bounds::new(point(px(10.), px(20.)), size(px(100.), px(60.)));
        assert_eq!(
            selection_quad_bounds(
                point(px(30.), px(20.)),
                point(px(70.), px(20.)),
                bounds,
                px(20.),
            ),
            vec![Bounds::from_corners(
                point(px(30.), px(20.)),
                point(px(70.), px(40.)),
            )]
        );
    }

    #[test]
    fn wrapped_selection_matches_text_view_quads() {
        let bounds = Bounds::new(point(px(10.), px(20.)), size(px(100.), px(100.)));
        assert_eq!(
            selection_quad_bounds(
                point(px(40.), px(20.)),
                point(px(30.), px(80.)),
                bounds,
                px(20.),
            ),
            vec![
                Bounds::from_corners(point(px(40.), px(20.)), point(px(110.), px(40.))),
                Bounds::from_corners(point(px(10.), px(40.)), point(px(110.), px(80.))),
                Bounds::from_corners(point(px(10.), px(80.)), point(px(30.), px(100.))),
            ]
        );
    }
}
