use std::{cell::RefCell, collections::HashMap, ops::Range, rc::Rc};

use gpui::{
    App, Bounds, ClipboardItem, CursorStyle, Element, ElementId, FocusHandle, FontWeight,
    GlobalElementId, HighlightStyle, Hitbox, HitboxBehavior, InspectorElementId, IntoElement,
    KeyDownEvent, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    Pixels, Point, Rgba, SharedString, StyledText, TextStyle, Window, fill, size,
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone)]
pub(crate) struct TextSelection {
    focus: FocusHandle,
    state: Rc<RefCell<SelectionState>>,
    regions: Rc<RefCell<HashMap<SharedString, Vec<TextRegion>>>>,
}

struct SelectionState {
    active_id: Option<SharedString>,
    source: SharedString,
    anchor: usize,
    selection: Range<usize>,
    selecting: bool,
    collecting: bool,
    pending_position: Option<Point<Pixels>>,
    #[cfg(target_os = "macos")]
    definition_visible: bool,
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
        #[cfg(target_os = "macos")]
        {
            self.definition_visible = false;
        }
    }
}

#[derive(Clone)]
struct TextRegion {
    layout: gpui::TextLayout,
    source_range: Range<usize>,
    hitbox: Hitbox,
}

pub(crate) struct SelectableText {
    id: SharedString,
    source: SharedString,
    source_range: Range<usize>,
    text: StyledText,
    highlights: Vec<AdaptiveHighlight>,
    selection: TextSelection,
    selection_color: Rgba,
}

pub(crate) struct AdaptiveHighlight {
    pub range: Range<usize>,
    pub style: HighlightStyle,
    pub missing_weight: Option<HighlightStyle>,
    pub missing_style: Option<HighlightStyle>,
}

pub(crate) struct PrepaintState {
    hitbox: Option<Hitbox>,
    selection: Vec<PaintQuad>,
}

#[cfg(target_os = "macos")]
pub(crate) fn configure_force_click(window: &Window) {
    dictionary::configure_force_click(window);
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
                #[cfg(target_os = "macos")]
                definition_visible: false,
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
        source_range: Range<usize>,
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
            let offset = source_range.start + nearest_index(&layout, position);
            state.active_id = Some(id.clone());
            state.source = source.clone();
            state.anchor = offset;
            state.selection = offset..offset;
            state.selecting = true;
        }
        drop(state);
        self.regions
            .borrow_mut()
            .entry(id)
            .or_default()
            .push(TextRegion {
                layout,
                source_range,
                hitbox,
            });
    }

    fn selected_range(
        &self,
        id: &SharedString,
        source: &SharedString,
        source_range: &Range<usize>,
    ) -> Range<usize> {
        let mut state = self.state.borrow_mut();
        if state.active_id.as_ref() != Some(id) {
            return 0..0;
        }
        if state.source != *source {
            state.clear();
            return 0..0;
        }
        let start = state.selection.start.max(source_range.start);
        let end = state.selection.end.min(source_range.end);
        if start >= end {
            0..0
        } else {
            (start - source_range.start)..(end - source_range.start)
        }
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
        #[cfg(target_os = "macos")]
        {
            state.definition_visible = false;
        }
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
        let Some(regions) = active_id.and_then(|id| self.regions.borrow().get(&id).cloned()) else {
            return;
        };
        let Some(region) = regions.iter().min_by(|left, right| {
            distance_to_bounds(left.hitbox.bounds, event.position)
                .total_cmp(&distance_to_bounds(right.hitbox.bounds, event.position))
        }) else {
            return;
        };
        let cursor = (region.source_range.start + nearest_index(&region.layout, event.position))
            .min(region.source_range.end);
        self.state.borrow_mut().selection = normalized_range(anchor, cursor);
        window.refresh();
    }

    pub(crate) fn mouse_up(&self, _: &MouseUpEvent, window: &mut Window) {
        let mut state = self.state.borrow_mut();
        let changed = state.collecting || state.selecting;
        state.collecting = false;
        state.selecting = false;
        state.pending_position = None;
        #[cfg(target_os = "macos")]
        {
            state.definition_visible = false;
        }
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

    #[cfg(target_os = "macos")]
    pub(crate) fn show_definition(
        &self,
        event: &gpui::MousePressureEvent,
        window: &Window,
    ) -> bool {
        if event.stage != gpui::PressureStage::Force {
            return false;
        }
        {
            let state = self.state.borrow();
            if state.definition_visible || !state.selecting {
                return false;
            }
        }

        let shown = dictionary::show_at(self, event.position, window);
        if shown {
            self.state.borrow_mut().definition_visible = true;
        }
        shown
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

pub(crate) fn selection_color(cx: &App) -> Rgba {
    crate::desktop::ui::theme::palette(cx).selection.into()
}

impl SelectableText {
    pub(crate) fn new(
        id: impl Into<SharedString>,
        source: impl Into<SharedString>,
        selection: TextSelection,
        selection_color: Rgba,
    ) -> Self {
        let source = source.into();
        let source_range = 0..source.len();
        Self {
            id: id.into(),
            text: StyledText::new(source.clone()),
            source,
            source_range,
            highlights: Vec::new(),
            selection,
            selection_color,
        }
    }

    pub(crate) fn fragment(
        id: impl Into<SharedString>,
        source: impl Into<SharedString>,
        source_range: Range<usize>,
        selection: TextSelection,
        selection_color: Rgba,
    ) -> Self {
        let source = source.into();
        let text = source
            .get(source_range.clone())
            .expect("selectable text fragment must be a valid source range");
        Self {
            id: id.into(),
            text: StyledText::new(text.to_string()),
            source,
            source_range,
            highlights: Vec::new(),
            selection,
            selection_color,
        }
    }

    pub(crate) fn with_highlights(
        mut self,
        highlights: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    ) -> Self {
        self.highlights.extend(
            highlights
                .into_iter()
                .map(|(range, style)| AdaptiveHighlight {
                    range,
                    style,
                    missing_weight: None,
                    missing_style: None,
                }),
        );
        self
    }

    pub(crate) fn with_adaptive_highlights(
        mut self,
        highlights: impl IntoIterator<Item = AdaptiveHighlight>,
    ) -> Self {
        self.highlights.extend(highlights);
        self
    }
}

fn missing_font_variant(
    text: &str,
    baseline: &TextStyle,
    target: &TextStyle,
    font_size: Pixels,
    window: &mut Window,
) -> bool {
    let mut indices = text
        .char_indices()
        .filter_map(|(index, character)| character.is_alphanumeric().then_some(index))
        .collect::<Vec<_>>();
    if indices.is_empty() {
        indices.extend(
            text.char_indices()
                .filter_map(|(index, character)| (!character.is_whitespace()).then_some(index)),
        );
    }
    if indices.is_empty() {
        return false;
    }

    let baseline =
        window
            .text_system()
            .layout_line(text, font_size, &[baseline.to_run(text.len())], None);
    let target =
        window
            .text_system()
            .layout_line(text, font_size, &[target.to_run(text.len())], None);
    indices.into_iter().any(|index| {
        baseline
            .font_id_for_index(index)
            .is_some_and(|font_id| target.font_id_for_index(index) == Some(font_id))
    })
}

#[cfg(target_os = "macos")]
mod dictionary;
mod element;
mod geometry;

use geometry::{distance_to_bounds, nearest_index, normalized_range};
