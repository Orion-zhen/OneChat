use super::*;

pub(super) fn nearest_index(layout: &gpui::TextLayout, position: gpui::Point<Pixels>) -> usize {
    layout
        .index_for_position(position)
        .unwrap_or_else(|index| index)
        .min(layout.len())
}

pub(super) fn normalized_range(anchor: usize, cursor: usize) -> Range<usize> {
    anchor.min(cursor)..anchor.max(cursor)
}

pub(super) fn distance_to_bounds(bounds: Bounds<Pixels>, position: Point<Pixels>) -> f32 {
    let dx = if position.x < bounds.left() {
        (bounds.left() - position.x).into()
    } else if position.x > bounds.right() {
        (position.x - bounds.right()).into()
    } else {
        0.0
    };
    let dy = if position.y < bounds.top() {
        (bounds.top() - position.y).into()
    } else if position.y > bounds.bottom() {
        (position.y - bounds.bottom()).into()
    } else {
        0.0
    };
    dx * dx + dy * dy
}

pub(super) fn selection_quads(
    layout: &gpui::TextLayout,
    text: &str,
    selection: &Range<usize>,
    color: Rgba,
) -> Vec<PaintQuad> {
    if selection.is_empty() {
        return Vec::new();
    }

    let selection = selection.start.min(text.len())..selection.end.min(text.len());
    let Some(selected_text) = text.get(selection.clone()) else {
        return Vec::new();
    };
    let bounds = layout.bounds();
    let line_height = layout.line_height();
    let mut quads: Vec<PaintQuad> = Vec::new();
    let mut current: Option<Bounds<Pixels>> = None;

    for (local_start, grapheme) in selected_text.grapheme_indices(true) {
        let start = selection.start + local_start;
        let end = start + grapheme.len();
        let Some(from) = layout.position_for_index(start) else {
            continue;
        };
        let Some(to) = layout.position_for_index(end) else {
            continue;
        };
        let width = if to.y == from.y {
            (to.x - from.x).max(gpui::px(1.0))
        } else {
            (bounds.right() - from.x).max(gpui::px(3.0))
        };
        let glyph_bounds = Bounds::new(from, size(width, line_height));

        if let Some(existing) = current.as_mut()
            && existing.top() == glyph_bounds.top()
            && (existing.right() - glyph_bounds.left()).abs() <= gpui::px(1.0)
        {
            existing.size.width = glyph_bounds.right() - existing.left();
        } else {
            if let Some(existing) = current.take() {
                quads.push(fill(existing, color));
            }
            current = Some(glyph_bounds);
        }
    }
    if let Some(existing) = current {
        quads.push(fill(existing, color));
    }
    quads
}
