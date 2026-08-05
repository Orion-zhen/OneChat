use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
    App, AvailableSpace, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    GlobalElementId, Hsla, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PaintQuad, Pixels, Point, Rgba, SharedString, Style, TextAlign, TextRun,
    UTF16Selection, UnderlineStyle, Window, WrappedLine, actions, div, fill, point, prelude::*, px,
    relative, rgb, rgba, size,
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy)]
struct InputPalette {
    text: Hsla,
    placeholder: Hsla,
    background: Rgba,
    border: Rgba,
    focused_border: Rgba,
    cursor: Rgba,
    selection: Rgba,
}

impl InputPalette {
    fn for_inherited_text(text: Hsla) -> Self {
        if text.l > 0.55 {
            Self {
                text,
                placeholder: rgb(0x9299a6).into(),
                background: rgb(0x1d2024),
                border: rgb(0x454b55),
                focused_border: rgb(0x7aa7ff),
                cursor: rgb(0x7aa7ff),
                selection: rgba(0x5b8cff52),
            }
        } else {
            Self {
                text,
                placeholder: rgb(0x9298a5).into(),
                background: rgb(0xffffff),
                border: rgb(0xcfd3da),
                focused_border: rgb(0x4f7fe8),
                cursor: rgb(0x2563eb),
                selection: rgba(0x3377ff38),
            }
        }
    }
}

actions!(
    composer,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        SelectHome,
        SelectEnd,
        Submit,
        Newline,
        Paste,
        Cut,
        Copy,
        ShowCharacterPalette,
        Cancel,
    ]
);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("Composer")),
        KeyBinding::new("delete", Delete, Some("Composer")),
        KeyBinding::new("left", Left, Some("Composer")),
        KeyBinding::new("right", Right, Some("Composer")),
        KeyBinding::new("up", Up, Some("Composer")),
        KeyBinding::new("down", Down, Some("Composer")),
        KeyBinding::new("shift-left", SelectLeft, Some("Composer")),
        KeyBinding::new("shift-right", SelectRight, Some("Composer")),
        KeyBinding::new("shift-up", SelectUp, Some("Composer")),
        KeyBinding::new("shift-down", SelectDown, Some("Composer")),
        KeyBinding::new("cmd-a", SelectAll, Some("Composer")),
        KeyBinding::new("cmd-left", Home, Some("Composer")),
        KeyBinding::new("cmd-right", End, Some("Composer")),
        KeyBinding::new("cmd-shift-left", SelectHome, Some("Composer")),
        KeyBinding::new("cmd-shift-right", SelectEnd, Some("Composer")),
        KeyBinding::new("enter", Submit, Some("Composer")),
        KeyBinding::new("shift-enter", Newline, Some("Composer")),
        KeyBinding::new("cmd-v", Paste, Some("Composer")),
        KeyBinding::new("cmd-x", Cut, Some("Composer")),
        KeyBinding::new("cmd-c", Copy, Some("Composer")),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some("Composer")),
        KeyBinding::new("escape", Cancel, Some("Composer")),
    ]);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposerEvent {
    Changed(String),
    Submit(String),
    Cancel,
}

pub struct Composer {
    focus_handle: FocusHandle,
    editor: EditorState,
    placeholder: SharedString,
    marked_range: Option<Range<usize>>,
    last_layout: Option<InputLayout>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    scroll_handle: gpui::ScrollHandle,
    single_line: bool,
    clear_on_submit: bool,
    read_only: bool,
}

impl Composer {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::configured("", "Message OneChat…", false, true, false, cx)
    }

    pub fn single_line(
        text: impl Into<String>,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::configured(text, placeholder, true, false, false, cx)
    }

    pub fn multiline(
        text: impl Into<String>,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::configured(text, placeholder, false, false, false, cx)
    }

    pub fn read_only(text: impl Into<String>, cx: &mut Context<Self>) -> Self {
        Self::configured(text, "", false, false, true, cx)
    }

    fn configured(
        text: impl Into<String>,
        placeholder: impl Into<SharedString>,
        single_line: bool,
        clear_on_submit: bool,
        read_only: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let text = text.into();
        let cursor = text.len();
        Self {
            focus_handle: cx.focus_handle(),
            editor: EditorState {
                text,
                selection: cursor..cursor,
                selection_reversed: false,
            },
            placeholder: placeholder.into(),
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            scroll_handle: gpui::ScrollHandle::new(),
            single_line,
            clear_on_submit,
            read_only,
        }
    }

    pub fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn text(&self) -> &str {
        &self.editor.text
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.editor.text = text.into();
        self.editor.move_to(self.editor.text.len());
        self.marked_range = None;
        self.changed(cx);
    }

    pub fn take_text(&mut self, cx: &mut Context<Self>) -> Option<String> {
        if self.editor.text.trim().is_empty() {
            return None;
        }
        let text = std::mem::take(&mut self.editor.text);
        self.editor.selection = 0..0;
        self.editor.selection_reversed = false;
        self.marked_range = None;
        self.changed(cx);
        Some(text)
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.scroll_handle.scroll_to_bottom();
        cx.emit(ComposerEvent::Changed(self.editor.text.clone()));
        cx.notify();
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.editor.move_to(offset);
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.editor.select_to(offset);
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let target = if self.editor.selection.is_empty() {
            self.editor.previous_boundary(self.editor.cursor())
        } else {
            self.editor.selection.start
        };
        self.move_to(target, cx);
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let target = if self.editor.selection.is_empty() {
            self.editor.next_boundary(self.editor.cursor())
        } else {
            self.editor.selection.end
        };
        self.move_to(target, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.editor.previous_boundary(self.editor.cursor()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.editor.next_boundary(self.editor.cursor()), cx);
    }

    fn move_vertical(&mut self, direction: f32, select: bool, cx: &mut Context<Self>) {
        let Some(layout) = self.last_layout.as_ref() else {
            return;
        };
        let cursor = layout.position_for_index(self.editor.cursor());
        let target = point(cursor.x, cursor.y + layout.line_height * direction);
        let offset = if target.y < px(0.0) {
            0
        } else if target.y >= layout.height {
            self.editor.text.len()
        } else {
            layout.index_for_position(target)
        };
        if select {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(-1.0, false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(1.0, false, cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(-1.0, true, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertical(1.0, true, cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.selection = 0..self.editor.text.len();
        self.editor.selection_reversed = false;
        cx.notify();
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.editor.line_start(), cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.editor.line_end(), cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.editor.line_start(), cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.editor.line_end(), cx);
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.editor.selection.is_empty() {
            let previous = self.editor.previous_boundary(self.editor.cursor());
            self.editor.select_to(previous);
        }
        self.editor.replace_selection("");
        self.marked_range = None;
        self.changed(cx);
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.editor.selection.is_empty() {
            let next = self.editor.next_boundary(self.editor.cursor());
            self.editor.select_to(next);
        }
        self.editor.replace_selection("");
        self.marked_range = None;
        self.changed(cx);
    }

    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        if self.single_line || self.read_only {
            return;
        }
        self.editor.replace_selection("\n");
        self.marked_range = None;
        self.changed(cx);
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        if self.read_only || self.editor.text.trim().is_empty() {
            return;
        }
        let text = self.editor.text.clone();
        if self.clear_on_submit {
            self.editor.text.clear();
            self.editor.selection = 0..0;
            self.editor.selection_reversed = false;
            self.marked_range = None;
            self.changed(cx);
        }
        cx.emit(ComposerEvent::Submit(text));
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(ComposerEvent::Cancel);
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.editor.replace_selection(&text.replace("\r\n", "\n"));
            self.marked_range = None;
            self.changed(cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.editor.selection.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.editor.text[self.editor.selection.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if !self.editor.selection.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.editor.text[self.editor.selection.clone()].to_string(),
            ));
            self.editor.replace_selection("");
            self.marked_range = None;
            self.changed(cx);
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let (Some(bounds), Some(layout)) = (self.last_bounds, self.last_layout.as_ref()) else {
            return 0;
        };
        layout.index_for_position(point(position.x - bounds.left(), position.y - bounds.top()))
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        self.is_selecting = true;
        let offset = self.index_for_mouse_position(event.position);
        if event.modifiers.shift {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }
}

impl EventEmitter<ComposerEvent> for Composer {}

impl Focusable for Composer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for Composer {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.editor.range_from_utf16(&range_utf16);
        actual_range.replace(self.editor.range_to_utf16(&range));
        Some(self.editor.text[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.editor.range_to_utf16(&self.editor.selection),
            reversed: self.editor.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.editor.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked_range = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range| self.editor.range_from_utf16(range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.editor.selection.clone());
        self.editor.replace_range(range, new_text);
        self.marked_range = None;
        self.changed(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range| self.editor.range_from_utf16(range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.editor.selection.clone());
        let start = range.start;
        self.editor.replace_range(range, new_text);

        self.marked_range = (!new_text.is_empty()).then_some(start..start + new_text.len());
        if let Some(selected) = new_selected_range_utf16 {
            let selected = range_from_utf16_in(new_text, &selected);
            self.editor.selection = start + selected.start..start + selected.end;
            self.editor.selection_reversed = false;
        }
        self.changed(cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let bounds = self.last_bounds?;
        let layout = self.last_layout.as_ref()?;
        let range = self.editor.range_from_utf16(&range_utf16);
        let start = layout.position_for_index(range.start);
        let end = layout.position_for_index(range.end);
        let width = if start.y == end.y {
            (end.x - start.x).max(px(1.0))
        } else {
            px(1.0)
        };
        Some(Bounds::new(
            point(bounds.left() + start.x, bounds.top() + start.y),
            size(width, layout.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let byte_index = self.index_for_mouse_position(position);
        Some(self.editor.offset_to_utf16(byte_index))
    }
}

impl Render for Composer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = InputPalette::for_inherited_text(window.text_style().color);
        div()
            .key_context("Composer")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .rounded_xl()
            .border_1()
            .border_color(if self.focus_handle.is_focused(window) {
                palette.focused_border
            } else {
                palette.border
            })
            .bg(palette.background)
            .text_color(palette.text)
            .p_3()
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cancel))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(
                div()
                    .id("composer-input-scroll")
                    .w_full()
                    .max_h(if self.single_line {
                        px(24.0)
                    } else {
                        px(196.0)
                    })
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .line_height(px(24.0))
                    .text_size(px(15.0))
                    .child(TextElement { input: cx.entity() }),
            )
    }
}

struct TextElement {
    input: Entity<Composer>,
}

#[derive(Clone, Default)]
struct RequestedLayout(Rc<RefCell<Option<InputLayout>>>);

struct PrepaintState {
    layout: InputLayout,
    cursor: PaintQuad,
    selection: Vec<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = RequestedLayout;
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
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();

        let input = self.input.clone();
        let text_style = window.text_style();
        let font = text_style.font();
        let text_color = text_style.color;
        let placeholder_color = InputPalette::for_inherited_text(text_color).placeholder;
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = text_style.line_height_in_pixels(window.rem_size());
        let requested_layout = RequestedLayout::default();
        let measured_layout = requested_layout.clone();

        let layout_id =
            window.request_measured_layout(style, move |known, available, window, cx| {
                let input = input.read(cx);
                let content: SharedString = input.editor.text.clone().into();
                let (display_text, color, marked_range) = if content.is_empty() {
                    (input.placeholder.clone(), placeholder_color, None)
                } else {
                    (content, text_color, input.marked_range.clone())
                };
                let display_len = display_text.len();
                let runs = text_runs(
                    TextRun {
                        len: display_len,
                        font: font.clone(),
                        color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    },
                    marked_range,
                );
                let wrap_width = known.width.or(match available.width {
                    AvailableSpace::Definite(width) => Some(width),
                    _ => None,
                });
                let lines = window
                    .text_system()
                    .shape_text(display_text, font_size, &runs, wrap_width, None)
                    .unwrap_or_default();
                let layout = InputLayout::new(lines, &input.editor.text, line_height);
                let content_width = layout.lines.iter().fold(px(0.0), |width, line| {
                    width.max(line.line.size(line_height).width)
                });
                let measured_size = size(
                    known.width.unwrap_or(content_width),
                    known.height.unwrap_or(layout.height.max(line_height)),
                );
                measured_layout.0.borrow_mut().replace(layout);
                measured_size
            });

        (layout_id, requested_layout)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        requested_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let text_style = window.text_style();
        let palette = InputPalette::for_inherited_text(text_style.color);
        let line_height = text_style.line_height_in_pixels(window.rem_size());
        let layout = requested_layout
            .0
            .borrow()
            .clone()
            .unwrap_or_else(|| InputLayout::empty(line_height, input.editor.text.len()));
        let cursor_position = layout.position_for_index(input.editor.cursor());
        let cursor = fill(
            Bounds::new(
                point(
                    bounds.left() + cursor_position.x,
                    bounds.top() + cursor_position.y,
                ),
                size(px(1.5), line_height),
            ),
            palette.cursor,
        );
        let selection = layout.selection_quads(bounds, &input.editor.selection, palette.selection);

        PrepaintState {
            layout,
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );

        for selection in prepaint.selection.drain(..) {
            window.paint_quad(selection);
        }
        for line in &prepaint.layout.lines {
            let _ = line.line.paint(
                point(bounds.left(), bounds.top() + line.y),
                prepaint.layout.line_height,
                TextAlign::Left,
                Some(bounds),
                window,
                cx,
            );
        }
        if focus_handle.is_focused(window) {
            window.paint_quad(prepaint.cursor.clone());
        }

        self.input.update(cx, |input, _| {
            input.last_layout = Some(prepaint.layout.clone());
            input.last_bounds = Some(bounds);
        });
    }
}

fn text_runs(base: TextRun, marked_range: Option<Range<usize>>) -> Vec<TextRun> {
    let Some(marked) = marked_range else {
        return vec![base];
    };
    let marked = marked.start.min(base.len)..marked.end.min(base.len);
    vec![
        TextRun {
            len: marked.start,
            ..base.clone()
        },
        TextRun {
            len: marked.end.saturating_sub(marked.start),
            underline: Some(UnderlineStyle {
                color: Some(base.color),
                thickness: px(1.0),
                wavy: false,
            }),
            ..base.clone()
        },
        TextRun {
            len: base.len.saturating_sub(marked.end),
            ..base
        },
    ]
    .into_iter()
    .filter(|run| run.len > 0)
    .collect()
}

#[derive(Clone)]
struct InputLayout {
    lines: Vec<InputLine>,
    line_height: Pixels,
    height: Pixels,
    content_len: usize,
}

#[derive(Clone)]
struct InputLine {
    line: WrappedLine,
    range: Range<usize>,
    y: Pixels,
    height: Pixels,
}

impl InputLayout {
    fn empty(line_height: Pixels, content_len: usize) -> Self {
        Self {
            lines: Vec::new(),
            line_height,
            height: line_height,
            content_len,
        }
    }

    fn new(
        lines: impl IntoIterator<Item = WrappedLine>,
        content: &str,
        line_height: Pixels,
    ) -> Self {
        let logical_ranges: Vec<Range<usize>> = if content.is_empty() {
            std::iter::once(0..0).collect()
        } else {
            let mut start = 0;
            content
                .split('\n')
                .map(|line| {
                    let range = start..start + line.len();
                    start = range.end + 1;
                    range
                })
                .collect()
        };

        let mut y = px(0.0);
        let mut input_lines = Vec::new();
        for (index, line) in lines.into_iter().enumerate() {
            let height = line.size(line_height).height.max(line_height);
            let range = logical_ranges
                .get(index)
                .cloned()
                .unwrap_or(content.len()..content.len());
            input_lines.push(InputLine {
                line,
                range,
                y,
                height,
            });
            y += height;
        }
        if input_lines.is_empty() {
            y = line_height;
        }

        Self {
            lines: input_lines,
            line_height,
            height: y,
            content_len: content.len(),
        }
    }

    fn position_for_index(&self, index: usize) -> Point<Pixels> {
        let index = index.min(self.content_len);
        let Some(line) = self
            .lines
            .iter()
            .find(|line| index <= line.range.end)
            .or_else(|| self.lines.last())
        else {
            return point(px(0.0), px(0.0));
        };
        let local = index.saturating_sub(line.range.start).min(line.line.len());
        let position = line
            .line
            .position_for_index(local, self.line_height)
            .unwrap_or_else(|| point(px(0.0), px(0.0)));
        point(position.x, line.y + position.y)
    }

    fn index_for_position(&self, position: Point<Pixels>) -> usize {
        if position.y < px(0.0) {
            return 0;
        }
        let Some(line) = self
            .lines
            .iter()
            .find(|line| position.y < line.y + line.height)
        else {
            return self.content_len;
        };
        let local_position = point(position.x, position.y - line.y);
        let local = line
            .line
            .closest_index_for_position(local_position, self.line_height)
            .unwrap_or_else(|index| index);
        (line.range.start + local).min(line.range.end)
    }

    fn selection_quads(
        &self,
        bounds: Bounds<Pixels>,
        selection: &Range<usize>,
        color: Rgba,
    ) -> Vec<PaintQuad> {
        if selection.is_empty() {
            return Vec::new();
        }
        let mut quads = Vec::new();
        for line in &self.lines {
            let mut starts = vec![0];
            let mut ends = Vec::new();
            for boundary in line.line.wrap_boundaries() {
                if let Some(glyph) = line
                    .line
                    .runs()
                    .get(boundary.run_ix)
                    .and_then(|run| run.glyphs.get(boundary.glyph_ix))
                {
                    ends.push(glyph.index);
                    starts.push(glyph.index);
                }
            }
            ends.push(line.line.len());

            for (visual_index, (segment_start, segment_end)) in
                starts.into_iter().zip(ends).enumerate()
            {
                let global_start = line.range.start + segment_start;
                let global_end = line.range.start + segment_end;
                let selected_start = selection.start.max(global_start);
                let selected_end = selection.end.min(global_end);
                if selected_start >= selected_end {
                    continue;
                }
                let line_start_x = line.line.unwrapped_layout.x_for_index(segment_start);
                let x1 = line
                    .line
                    .unwrapped_layout
                    .x_for_index(selected_start - line.range.start)
                    - line_start_x;
                let x2 = line
                    .line
                    .unwrapped_layout
                    .x_for_index(selected_end - line.range.start)
                    - line_start_x;
                quads.push(fill(
                    Bounds::new(
                        point(
                            bounds.left() + x1,
                            bounds.top() + line.y + self.line_height * visual_index as f32,
                        ),
                        size((x2 - x1).max(px(1.0)), self.line_height),
                    ),
                    color,
                ));
            }

            let newline_is_selected = line.range.end < self.content_len
                && selection.start <= line.range.end
                && selection.end > line.range.end;
            if newline_is_selected {
                let position = line
                    .line
                    .position_for_index(line.line.len(), self.line_height)
                    .unwrap_or_else(|| point(px(0.0), line.height - self.line_height));
                quads.push(fill(
                    Bounds::new(
                        point(
                            bounds.left() + position.x,
                            bounds.top() + line.y + position.y,
                        ),
                        size(
                            (bounds.size.width - position.x).max(px(3.0)),
                            self.line_height,
                        ),
                    ),
                    color,
                ));
            }
        }
        quads
    }
}

#[derive(Debug, Default)]
struct EditorState {
    text: String,
    selection: Range<usize>,
    selection_reversed: bool,
}

impl EditorState {
    fn cursor(&self) -> usize {
        if self.selection_reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    fn move_to(&mut self, offset: usize) {
        let offset = self.clamp_boundary(offset);
        self.selection = offset..offset;
        self.selection_reversed = false;
    }

    fn select_to(&mut self, offset: usize) {
        let offset = self.clamp_boundary(offset);
        let anchor = if self.selection_reversed {
            self.selection.end
        } else {
            self.selection.start
        };
        self.selection = anchor.min(offset)..anchor.max(offset);
        self.selection_reversed = offset < anchor;
    }

    fn replace_selection(&mut self, new_text: &str) {
        self.replace_range(self.selection.clone(), new_text);
    }

    fn replace_range(&mut self, range: Range<usize>, new_text: &str) {
        let range = self.clamp_boundary(range.start)..self.clamp_boundary(range.end);
        self.text.replace_range(range.clone(), new_text);
        self.move_to(range.start + new_text.len());
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.text.len())
    }

    fn line_start(&self) -> usize {
        let cursor = self.cursor();
        self.text[..cursor].rfind('\n').map_or(0, |index| index + 1)
    }

    fn line_end(&self) -> usize {
        let cursor = self.cursor();
        self.text[cursor..]
            .find('\n')
            .map_or(self.text.len(), |index| cursor + index)
    }

    fn clamp_boundary(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        offset_from_utf16_in(&self.text, offset)
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text[..self.clamp_boundary(offset)]
            .encode_utf16()
            .count()
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }
}

fn offset_from_utf16_in(text: &str, offset: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_offset = 0;
    for ch in text.chars() {
        if utf16_offset >= offset {
            break;
        }
        utf16_offset += ch.len_utf16();
        utf8_offset += ch.len_utf8();
    }
    utf8_offset
}

fn range_from_utf16_in(text: &str, range: &Range<usize>) -> Range<usize> {
    offset_from_utf16_in(text, range.start)..offset_from_utf16_in(text, range.end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_palette_keeps_background_contrasted_with_inherited_text() {
        let dark = InputPalette::for_inherited_text(rgb(0xf3f4f6).into());
        assert_eq!(dark.background, rgb(0x1d2024));
        assert_eq!(dark.placeholder, rgb(0x9299a6).into());
        assert_ne!(dark.background, rgb(0xffffff));

        let light = InputPalette::for_inherited_text(rgb(0x202124).into());
        assert_eq!(light.background, rgb(0xffffff));
        assert_eq!(light.placeholder, rgb(0x9298a5).into());
    }

    #[test]
    fn translates_utf16_ranges_for_ime_text() {
        let editor = EditorState {
            text: "a😀文b".into(),
            selection: 0..0,
            selection_reversed: false,
        };

        assert_eq!(editor.offset_from_utf16(0), 0);
        assert_eq!(editor.offset_from_utf16(1), 1);
        assert_eq!(editor.offset_from_utf16(3), 5);
        assert_eq!(editor.offset_from_utf16(4), 8);
        assert_eq!(editor.range_to_utf16(&(1..8)), 1..4);
    }

    #[test]
    fn replacement_updates_selection_and_preserves_newlines() {
        let mut editor = EditorState {
            text: "hello world".into(),
            selection: 6..11,
            selection_reversed: false,
        };

        editor.replace_selection("GPUI\nchat");

        assert_eq!(editor.text, "hello GPUI\nchat");
        assert_eq!(editor.selection, editor.text.len()..editor.text.len());
    }

    #[test]
    fn cursor_navigation_uses_grapheme_boundaries() {
        let mut editor = EditorState {
            text: "a👋🏽b".into(),
            selection: 0..0,
            selection_reversed: false,
        };
        editor.move_to(1);

        let after_emoji = editor.next_boundary(editor.cursor());
        assert_eq!(&editor.text[1..after_emoji], "👋🏽");
        assert_eq!(editor.previous_boundary(after_emoji), 1);
    }

    #[test]
    fn reversed_selection_keeps_a_stable_anchor() {
        let mut editor = EditorState {
            text: "abcdef".into(),
            selection: 4..4,
            selection_reversed: false,
        };

        editor.select_to(2);
        editor.select_to(1);
        assert_eq!(editor.selection, 1..4);
        assert!(editor.selection_reversed);

        editor.select_to(5);
        assert_eq!(editor.selection, 4..5);
        assert!(!editor.selection_reversed);
    }
}
