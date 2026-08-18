use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    ops::Range,
    rc::{Rc, Weak},
    time::Duration,
};

use gpui::{
    AnyElement, App, Bounds, CursorStyle, ElementId, Font, FontWeight, HighlightStyle, PaintQuad,
    Pixels, Point, Rgba, ScrollHandle, SharedString, StyledText, TextStyle, Window, canvas, div,
    fill, prelude::*,
};
use gpui_base::{
    ElementExt as _, TextSelectionCoverage, TextSelectionEvent, TextSelectionHandle,
    TextSelectionRegistration, TextSelectionRun,
};

#[cfg(target_os = "macos")]
use crate::desktop::pressure_touch::{ForceClickChange, ForceClickState};

#[derive(Clone)]
pub(crate) struct TextSelection {
    registry: Rc<RefCell<SelectionRegistry>>,
    message_scroll: Rc<RefCell<Option<ScrollHandle>>>,
    auto_scroll: Rc<RefCell<AutoScrollState>>,
    #[cfg(target_os = "macos")]
    force_click: Rc<RefCell<ForceClickState>>,
}

#[derive(Default)]
struct AutoScrollState {
    delta: Option<Pixels>,
    running: bool,
}

#[derive(Default)]
struct SelectionRegistry {
    generation: u64,
    next_document_order: u64,
    groups: HashMap<SharedString, GroupEntry>,
}

struct GroupEntry {
    handle: Option<TextSelectionHandle>,
    runtime: Rc<RefCell<GroupRuntime>>,
    last_seen: u64,
    document_order: u64,
}

#[derive(Default)]
struct GroupRuntime {
    generation: u64,
    section_separator: SharedString,
    runs: Vec<TextSelectionRun>,
    bounds: Option<Bounds<Pixels>>,
    text_bounds: Vec<Bounds<Pixels>>,
    selected: BTreeMap<(u64, u64, usize), String>,
    #[cfg(target_os = "macos")]
    regions: Vec<TextRegion>,
}

impl GroupRuntime {
    fn begin_frame(&mut self, generation: u64, section_separator: SharedString) {
        if self.generation == generation {
            return;
        }
        self.generation = generation;
        self.section_separator = section_separator;
        self.runs.clear();
        self.text_bounds.clear();
        self.selected.clear();
        #[cfg(target_os = "macos")]
        self.regions.clear();
    }

    fn selected_text(&self) -> String {
        let mut sections: BTreeMap<u64, String> = BTreeMap::new();
        for ((section, _, _), text) in &self.selected {
            sections.entry(*section).or_default().push_str(text);
        }
        sections
            .into_values()
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(&self.section_separator)
    }
}

#[derive(Clone)]
pub(crate) struct SelectionGroup {
    key: SharedString,
    document_order: u64,
    generation: u64,
    section_separator: SharedString,
    selection: TextSelection,
}

#[cfg(target_os = "macos")]
#[derive(Clone)]
struct TextRegion {
    layout: gpui::TextLayout,
    source: SharedString,
    source_range: Range<usize>,
    font: Font,
    bounds: Bounds<Pixels>,
}

pub(crate) struct SelectableText {
    group: SelectionGroup,
    order: u64,
    section: u64,
    source: SharedString,
    source_range: Range<usize>,
    text: StyledText,
    highlights: Vec<AdaptiveHighlight>,
    selection_color: Rgba,
}

pub(crate) struct AdaptiveHighlight {
    pub range: Range<usize>,
    pub variant_range: Range<usize>,
    pub style: HighlightStyle,
    pub missing_weight: Option<HighlightStyle>,
    pub missing_style: Option<HighlightStyle>,
}

pub(crate) struct PrepaintState;

impl TextSelection {
    pub(crate) fn new() -> Self {
        Self {
            registry: Rc::new(RefCell::new(SelectionRegistry::default())),
            message_scroll: Rc::new(RefCell::new(None)),
            auto_scroll: Rc::new(RefCell::new(AutoScrollState::default())),
            #[cfg(target_os = "macos")]
            force_click: Rc::new(RefCell::new(ForceClickState::default())),
        }
    }

    pub(crate) fn begin_frame(&self, message_scroll: ScrollHandle) {
        *self.message_scroll.borrow_mut() = Some(message_scroll);
        let mut registry = self.registry.borrow_mut();
        let previous = registry.generation;
        registry
            .groups
            .retain(|_, entry| entry.last_seen == previous);
        registry.generation = registry.generation.wrapping_add(1).max(1);
        registry.next_document_order = 0;
    }

    pub(crate) fn group(&self, key: impl Into<SharedString>) -> SelectionGroup {
        self.group_with_separator(key, "")
    }

    pub(crate) fn group_with_separator(
        &self,
        key: impl Into<SharedString>,
        separator: impl Into<SharedString>,
    ) -> SelectionGroup {
        let key = key.into();
        let section_separator = separator.into();
        let mut registry = self.registry.borrow_mut();
        let generation = registry.generation;
        if !registry.groups.contains_key(&key) {
            registry.groups.insert(
                key.clone(),
                GroupEntry {
                    handle: None,
                    runtime: Rc::new(RefCell::new(GroupRuntime::default())),
                    last_seen: 0,
                    document_order: 0,
                },
            );
        }
        let needs_order = registry
            .groups
            .get(&key)
            .is_some_and(|entry| entry.last_seen != generation);
        if needs_order {
            let order = registry.next_document_order;
            registry.next_document_order += 1;
            let entry = registry.groups.get_mut(&key).unwrap();
            entry.last_seen = generation;
            entry.document_order = order;
        }
        let entry = registry.groups.get(&key).unwrap();
        let document_order = entry.document_order;
        SelectionGroup {
            key,
            document_order,
            generation,
            section_separator,
            selection: self.clone(),
        }
    }

    pub(crate) fn clear(&self, window: &mut Window, cx: &mut App) {
        gpui_base::TextSelection::clear(window, cx);
        for entry in self.registry.borrow().groups.values() {
            entry.runtime.borrow_mut().selected.clear();
        }
        self.auto_scroll.borrow_mut().delta = None;
        #[cfg(target_os = "macos")]
        self.force_click.borrow_mut().cancel();
        window.refresh();
    }

    fn handle(&self, group: &SelectionGroup, window: &Window, cx: &mut App) -> TextSelectionHandle {
        let (runtime, existing) = {
            let registry = self.registry.borrow();
            let entry = registry
                .groups
                .get(&group.key)
                .expect("selection group must be allocated before paint");
            (entry.runtime.clone(), entry.handle.clone())
        };
        if let Some(handle) = existing {
            return handle;
        }

        let handle = TextSelectionHandle::new("", cx);
        let weak_runtime: Weak<RefCell<GroupRuntime>> = Rc::downgrade(&runtime);
        handle.copy_with(
            move |_| {
                weak_runtime
                    .upgrade()
                    .map(|runtime| runtime.borrow().selected_text())
                    .unwrap_or_default()
            },
            cx,
        );
        handle.refresh_window_on_change(window, cx).detach();

        let scroll = self.message_scroll.clone();
        let auto_scroll = self.auto_scroll.clone();
        let auto_scroll_runtime = Rc::downgrade(&runtime);
        let window_handle = window.window_handle();
        handle
            .subscribe(
                move |event, cx| {
                    let TextSelectionEvent::AutoScroll(delta) = event else {
                        return;
                    };
                    let delta = delta.filter(|delta| {
                        let Some(scroll) = scroll.borrow().clone() else {
                            return false;
                        };
                        let Some(runtime) = auto_scroll_runtime.upgrade() else {
                            return false;
                        };
                        let Some(participant) = runtime.borrow().bounds else {
                            return false;
                        };
                        let viewport = scroll.bounds();
                        if *delta > gpui::px(0.) {
                            participant.bottom() >= viewport.bottom() - gpui::px(16.)
                        } else {
                            participant.top() <= viewport.top() + gpui::px(16.)
                        }
                    });
                    let should_start = {
                        let mut state = auto_scroll.borrow_mut();
                        state.delta = delta;
                        let start = delta.is_some() && !state.running;
                        state.running |= start;
                        start
                    };
                    if !should_start {
                        return;
                    }
                    let scroll = scroll.clone();
                    let auto_scroll = Rc::downgrade(&auto_scroll);
                    let window_handle = window_handle.clone();
                    cx.spawn(async move |cx| {
                        loop {
                            cx.background_executor()
                                .timer(Duration::from_millis(16))
                                .await;
                            let Some(state) = auto_scroll.upgrade() else {
                                break;
                            };
                            let Some(delta) = state.borrow().delta else {
                                state.borrow_mut().running = false;
                                break;
                            };
                            if let Some(scroll) = scroll.borrow().as_ref() {
                                let mut offset = scroll.offset();
                                offset.y -= delta;
                                scroll.set_offset(offset);
                                _ = window_handle.update(cx, |_, window, _| window.refresh());
                            }
                        }
                    })
                    .detach();
                },
                cx,
            )
            .detach();

        self.registry
            .borrow_mut()
            .groups
            .get_mut(&group.key)
            .expect("selection group disappeared during paint")
            .handle = Some(handle.clone());
        handle
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn show_definition(
        &self,
        event: &gpui::MousePressureEvent,
        window: &Window,
    ) -> bool {
        if self.force_click.borrow_mut().update(event) != ForceClickChange::Triggered {
            return false;
        }
        dictionary::show_at(self, event.position, window)
    }
}

impl SelectionGroup {
    pub(crate) fn wrap(&self, content: impl IntoElement) -> AnyElement {
        let group = self.clone();
        div()
            .relative()
            .on_prepaint(move |bounds, window, cx| group.register(bounds, window, cx))
            .child(content)
            .into_any_element()
    }

    fn register(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let runtime = self
            .selection
            .registry
            .borrow()
            .groups
            .get(&self.key)
            .expect("selection group must remain live during prepaint")
            .runtime
            .clone();
        let text_bounds = runtime.borrow().text_bounds.clone();
        runtime
            .borrow_mut()
            .begin_frame(self.generation, self.section_separator.clone());
        runtime.borrow_mut().bounds = Some(bounds);
        let handle = self.selection.handle(self, window, cx);
        let hitbox = window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal);
        if hitbox.is_hovered(window) {
            window.set_cursor_style(CursorStyle::IBeam, &hitbox);
        }
        handle.register(
            TextSelectionRegistration::new(hitbox, bounds)
                .with_document_order(self.document_order)
                .with_text_bounds(if text_bounds.is_empty() {
                    vec![bounds]
                } else {
                    text_bounds
                }),
            window,
            cx,
        );
    }

    fn runtime_and_handle(
        &self,
        window: &Window,
        cx: &mut App,
    ) -> (Rc<RefCell<GroupRuntime>>, TextSelectionHandle) {
        let handle = self.selection.handle(self, window, cx);
        let runtime = self
            .selection
            .registry
            .borrow()
            .groups
            .get(&self.key)
            .expect("selection group must remain live during paint")
            .runtime
            .clone();
        runtime
            .borrow_mut()
            .begin_frame(self.generation, self.section_separator.clone());
        (runtime, handle)
    }

    fn project_text(
        &self,
        order: u64,
        section: u64,
        source_start: usize,
        text: SharedString,
        layout: gpui::TextLayout,
        bounds: Bounds<Pixels>,
        window: &Window,
        cx: &mut App,
    ) -> Option<Range<usize>> {
        let (runtime, handle) = self.runtime_and_handle(window, cx);
        let run_index = {
            let mut runtime = runtime.borrow_mut();
            let index = runtime.runs.len();
            runtime.runs.push(
                TextSelectionRun::new(text.clone(), layout, bounds)
                    .with_document_order(index as u64),
            );
            index
        };
        let runs = runtime.borrow().runs.clone();
        let projection = handle.update_runs(&runs, cx);
        let range = projection.ranges().get(run_index).cloned().flatten();
        let mut runtime = runtime.borrow_mut();
        runtime.selected.remove(&(section, order, source_start));
        if let Some(range) = range.clone() {
            runtime
                .selected
                .insert((section, order, source_start), text[range].to_string());
        }
        range
    }

    fn register_text_bounds(&self, bounds: Bounds<Pixels>) {
        let runtime = self
            .selection
            .registry
            .borrow()
            .groups
            .get(&self.key)
            .expect("selection group must remain live during prepaint")
            .runtime
            .clone();
        runtime
            .borrow_mut()
            .begin_frame(self.generation, self.section_separator.clone());
        runtime.borrow_mut().text_bounds.push(bounds);
    }

    #[cfg(target_os = "macos")]
    fn register_region(
        &self,
        source: SharedString,
        source_range: Range<usize>,
        layout: gpui::TextLayout,
        font: Font,
        bounds: Bounds<Pixels>,
    ) {
        let runtime = self
            .selection
            .registry
            .borrow()
            .groups
            .get(&self.key)
            .expect("selection group must remain live during prepaint")
            .runtime
            .clone();
        runtime.borrow_mut().regions.push(TextRegion {
            layout,
            source,
            source_range,
            font,
            bounds,
        });
    }

    pub(crate) fn atom(
        &self,
        order: u64,
        section: u64,
        text: impl Into<SharedString>,
        selection_color: Rgba,
        content: impl IntoElement,
    ) -> AnyElement {
        let text = text.into();
        let selected = Rc::new(RefCell::new(false));
        let prepaint_group = self.clone();
        let prepaint_text = text.clone();
        let prepaint_selected = selected.clone();
        let paint_selected = selected;
        div()
            .relative()
            .child(
                canvas(
                    move |bounds, window, cx| {
                        prepaint_group.register_text_bounds(bounds);
                        let (runtime, handle) = prepaint_group.runtime_and_handle(window, cx);
                        let is_selected = handle
                            .snapshot(cx)
                            .is_some_and(|snapshot| atom_is_selected(snapshot, bounds));
                        *prepaint_selected.borrow_mut() = is_selected;
                        let mut runtime = runtime.borrow_mut();
                        runtime.selected.remove(&(section, order, 0));
                        if is_selected {
                            runtime
                                .selected
                                .insert((section, order, 0), prepaint_text.to_string());
                        }
                    },
                    move |bounds, _, window, _| {
                        if *paint_selected.borrow() {
                            window.paint_quad(fill(bounds, selection_color));
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .child(content)
            .into_any_element()
    }
}

fn atom_is_selected(snapshot: gpui_base::TextSelectionSnapshot, bounds: Bounds<Pixels>) -> bool {
    if snapshot.coverage() == TextSelectionCoverage::Full {
        return true;
    }
    let Some(points) = snapshot.window_points() else {
        return false;
    };
    let center = bounds.center();
    let start = points.anchor();
    let end = points.cursor();
    let line_height = bounds.size.height;
    let same_line = (start.y - end.y).abs() < line_height;
    if same_line {
        let left = start.x.min(end.x);
        let right = start.x.max(end.x);
        center.y >= start.y.min(end.y) - line_height
            && center.y <= start.y.max(end.y) + line_height
            && center.x >= left
            && center.x <= right
    } else {
        let (top, bottom) = if start.y <= end.y {
            (start, end)
        } else {
            (end, start)
        };
        center.y > top.y && center.y < bottom.y
            || (center.y - top.y).abs() < line_height && center.x >= top.x
            || (center.y - bottom.y).abs() < line_height && center.x <= bottom.x
    }
}

pub(crate) fn selection_color(cx: &App) -> Rgba {
    crate::desktop::ui::theme::palette(cx).selection.into()
}

impl SelectableText {
    pub(crate) fn new(
        group: SelectionGroup,
        order: u64,
        source: impl Into<SharedString>,
        selection_color: Rgba,
    ) -> Self {
        let source = source.into();
        let source_range = 0..source.len();
        Self {
            group,
            order,
            section: 0,
            text: StyledText::new(source.clone()),
            source,
            source_range,
            highlights: Vec::new(),
            selection_color,
        }
    }

    pub(crate) fn fragment(
        group: SelectionGroup,
        order: u64,
        source: impl Into<SharedString>,
        source_range: Range<usize>,
        selection_color: Rgba,
    ) -> Self {
        let source = source.into();
        let text = source
            .get(source_range.clone())
            .expect("selectable text fragment must be a valid source range");
        Self {
            group,
            order,
            section: 0,
            text: StyledText::new(text.to_string()),
            source,
            source_range,
            highlights: Vec::new(),
            selection_color,
        }
    }

    pub(crate) fn section(mut self, section: u64) -> Self {
        self.section = section;
        self
    }

    pub(crate) fn with_highlights(
        mut self,
        highlights: impl IntoIterator<Item = (Range<usize>, HighlightStyle)>,
    ) -> Self {
        self.highlights.extend(
            highlights
                .into_iter()
                .map(|(range, style)| AdaptiveHighlight {
                    variant_range: range.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_parts_are_concatenated_by_section() {
        let mut runtime = GroupRuntime {
            section_separator: "\t".into(),
            ..Default::default()
        };
        runtime.selected.insert((1, 3, 0), "right".into());
        runtime.selected.insert((0, 1, 0), "left".into());
        runtime.selected.insert((0, 2, 0), " side".into());
        assert_eq!(runtime.selected_text(), "left side\tright");
    }

    #[test]
    fn formula_source_stays_atomic_in_rendered_copy() {
        let mut runtime = GroupRuntime::default();
        runtime.selected.insert((0, 0, 0), "before ".into());
        runtime.selected.insert((0, 1, 0), "x^2".into());
        runtime.selected.insert((0, 2, 0), " after".into());
        assert_eq!(runtime.selected_text(), "before x^2 after");
    }
}
