use std::{cell::RefCell, collections::HashMap, ops::Range, rc::Rc};

use gpui::{
    App, Bounds, ClipboardItem, CursorStyle, Element, ElementId, FocusHandle, GlobalElementId,
    HighlightStyle, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, KeyDownEvent,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    Rgba, SharedString, StyledText, Window, fill, rgba, size,
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone)]
pub(crate) struct TextSelection {
    focus: FocusHandle,
    state: Rc<RefCell<SelectionState>>,
    regions: Rc<RefCell<HashMap<SharedString, TextRegion>>>,
}

struct SelectionState {
    active_id: Option<SharedString>,
    source: SharedString,
    anchor: usize,
    selection: Range<usize>,
    selecting: bool,
    collecting: bool,
    pending_position: Option<Point<Pixels>>,
}

impl SelectionState {
    fn clear(&mut self) {
        self.active_id = None;
        self.source = "".into();
        self.anchor = 0;
        self.selection = 0..0;
        self.selecting = false;
        self.collecting = false;
        self.pending_position = None;
    }
}

#[derive(Clone)]
struct TextRegion {
    layout: gpui::TextLayout,
}

pub(crate) struct SelectableText {
    id: SharedString,
    source: SharedString,
    text: StyledText,
    selection: TextSelection,
    selection_color: Rgba,
}

pub(crate) struct PrepaintState {
    hitbox: Option<Hitbox>,
    selection: Vec<PaintQuad>,
}

impl TextSelection {
    pub(crate) fn new(focus: FocusHandle) -> Self {
        Self {
            focus,
            state: Rc::new(RefCell::new(SelectionState {
                active_id: None,
                source: "".into(),
                anchor: 0,
                selection: 0..0,
                selecting: false,
                collecting: false,
                pending_position: None,
            })),
            regions: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub(crate) fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    pub(crate) fn begin_frame(&self) {
        self.regions.borrow_mut().clear();
    }

    pub(crate) fn clear(&self, window: &mut Window) {
        self.state.borrow_mut().clear();
        self.regions.borrow_mut().clear();
        window.refresh();
    }

    fn clear_if_unfocused(&self, window: &Window) {
        if !self.focus.is_focused(window) {
            self.state.borrow_mut().clear();
            self.regions.borrow_mut().clear();
        }
    }

    fn is_collecting(&self) -> bool {
        self.state.borrow().collecting
    }

    fn register(
        &self,
        id: SharedString,
        source: SharedString,
        layout: gpui::TextLayout,
        hitbox: Hitbox,
    ) {
        let mut state = self.state.borrow_mut();
        if !state.collecting {
            return;
        }
        if state.pending_position.is_some_and(|position| {
            hitbox.bounds.contains(&position) && hitbox.content_mask.bounds.contains(&position)
        }) {
            let position = state.pending_position.take().unwrap();
            let offset = nearest_index(&layout, position);
            state.active_id = Some(id.clone());
            state.source = source.clone();
            state.anchor = offset;
            state.selection = offset..offset;
            state.selecting = true;
        }
        drop(state);
        self.regions.borrow_mut().insert(id, TextRegion { layout });
    }

    fn selected_range(&self, id: &SharedString, source: &SharedString) -> Range<usize> {
        let mut state = self.state.borrow_mut();
        if state.active_id.as_ref() != Some(id) {
            return 0..0;
        }
        if state.source != *source {
            state.clear();
            return 0..0;
        }
        state.selection.clone()
    }

    pub(crate) fn mouse_down(&self, event: &MouseDownEvent, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
        let mut state = self.state.borrow_mut();
        state.active_id = None;
        state.source = "".into();
        state.anchor = 0;
        state.selection = 0..0;
        state.selecting = false;
        state.collecting = true;
        state.pending_position = Some(event.position);
        self.regions.borrow_mut().clear();
        window.refresh();
        cx.stop_propagation();
    }

    pub(crate) fn mouse_move(&self, event: &MouseMoveEvent, window: &mut Window) {
        if event.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let (active_id, anchor) = {
            let state = self.state.borrow();
            if !state.selecting {
                return;
            }
            (state.active_id.clone(), state.anchor)
        };
        let Some(region) = active_id.and_then(|id| self.regions.borrow().get(&id).cloned()) else {
            return;
        };
        let cursor = nearest_index(&region.layout, event.position);
        self.state.borrow_mut().selection = normalized_range(anchor, cursor);
        window.refresh();
    }

    pub(crate) fn mouse_up(&self, _: &MouseUpEvent, window: &mut Window) {
        let mut state = self.state.borrow_mut();
        let changed = state.collecting || state.selecting;
        state.collecting = false;
        state.selecting = false;
        state.pending_position = None;
        if state.selection.is_empty() {
            state.active_id = None;
            state.source = "".into();
        }
        drop(state);
        self.regions.borrow_mut().clear();
        if changed {
            window.refresh();
        }
    }

    pub(crate) fn copy(&self, event: &KeyDownEvent, window: &Window, cx: &mut App) {
        let copy_modifier = if cfg!(target_os = "macos") {
            event.keystroke.modifiers.platform
        } else {
            event.keystroke.modifiers.control
        };
        if !self.focus.is_focused(window) || !copy_modifier || event.keystroke.key != "c" {
            return;
        }
        let state = self.state.borrow();
        if !state.selection.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                state.source[state.selection.clone()].to_string(),
            ));
            cx.stop_propagation();
        }
    }
}

pub(crate) fn selection_color(dark: bool) -> Rgba {
    if dark {
        rgba(0x0a84ff52)
    } else {
        rgba(0x007aff38)
    }
}

impl SelectableText {
    pub(crate) fn new(
        id: impl Into<SharedString>,
        source: impl Into<SharedString>,
        selection: TextSelection,
        selection_color: Rgba,
    ) -> Self {
        let source = source.into();
        Self {
            id: id.into(),
            text: StyledText::new(source.clone()),
            source,
            selection,
            selection_color,
        }
    }

    pub(crate) fn with_highlights(
        mut self,
        highlights: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    ) -> Self {
        let highlights = highlights.into_iter().collect::<Vec<_>>();
        if !highlights.is_empty() {
            self.text = self.text.with_highlights(highlights);
        }
        self
    }
}

impl IntoElement for SelectableText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SelectableText {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.text.request_layout(None, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.text
            .prepaint(None, inspector_id, bounds, request_state, window, cx);
        self.selection.clear_if_unfocused(window);
        let selected_range = self.selection.selected_range(&self.id, &self.source);
        let hitbox = self.selection.is_collecting().then(|| {
            let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
            self.selection.register(
                self.id.clone(),
                self.source.clone(),
                self.text.layout().clone(),
                hitbox.clone(),
            );
            hitbox
        });
        let selection = selection_quads(
            self.text.layout(),
            &self.source,
            &selected_range,
            self.selection_color,
        );
        PrepaintState { hitbox, selection }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(hitbox) = prepaint.hitbox.as_ref()
            && hitbox.is_hovered(window)
        {
            window.set_cursor_style(CursorStyle::IBeam, hitbox);
        }
        for quad in prepaint.selection.drain(..) {
            window.paint_quad(quad);
        }
        self.text
            .paint(None, inspector_id, bounds, &mut (), &mut (), window, cx);
    }
}

fn nearest_index(layout: &gpui::TextLayout, position: gpui::Point<Pixels>) -> usize {
    layout
        .index_for_position(position)
        .unwrap_or_else(|index| index)
        .min(layout.len())
}

fn normalized_range(anchor: usize, cursor: usize) -> Range<usize> {
    anchor.min(cursor)..anchor.max(cursor)
}

fn selection_quads(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_range_is_order_independent() {
        assert_eq!(normalized_range(2, 7), 2..7);
        assert_eq!(normalized_range(7, 2), 2..7);
    }

    #[test]
    fn clearing_selection_resets_drag_and_copy_state() {
        let mut state = SelectionState {
            active_id: Some("message".into()),
            source: "selected text".into(),
            anchor: 2,
            selection: 2..8,
            selecting: true,
            collecting: true,
            pending_position: Some(Point::default()),
        };

        state.clear();

        assert!(state.active_id.is_none());
        assert!(state.source.is_empty());
        assert_eq!(state.anchor, 0);
        assert!(state.selection.is_empty());
        assert!(!state.selecting);
        assert!(!state.collecting);
        assert!(state.pending_position.is_none());
    }
}
