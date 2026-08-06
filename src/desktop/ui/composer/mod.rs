mod editor;
mod element;
mod input;
mod layout;
mod render;

use editor::{EditorState, range_from_utf16_in};
use element::TextElement;
use layout::{InputLayout, estimated_visual_lines, text_runs};

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
    prominent_background: Rgba,
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
                placeholder: rgb(0x8e8e93).into(),
                background: rgba(0xffffff10),
                prominent_background: rgba(0x2c2c2ef7),
                border: rgba(0xffffff18),
                focused_border: rgb(0x0a84ff),
                cursor: rgb(0x0a84ff),
                selection: rgba(0x0a84ff52),
            }
        } else {
            Self {
                text,
                placeholder: rgb(0x8e8e93).into(),
                background: rgba(0x76768014),
                prominent_background: rgba(0xfffffffa),
                border: rgba(0x3c3c431f),
                focused_border: rgb(0x007aff),
                cursor: rgb(0x007aff),
                selection: rgba(0x007aff38),
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
    let primary = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    let shortcut = |key: &str| format!("{primary}-{key}");
    let mut bindings = vec![
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
        KeyBinding::new(&shortcut("a"), SelectAll, Some("Composer")),
        KeyBinding::new(&shortcut("left"), Home, Some("Composer")),
        KeyBinding::new(&shortcut("right"), End, Some("Composer")),
        KeyBinding::new(&shortcut("shift-left"), SelectHome, Some("Composer")),
        KeyBinding::new(&shortcut("shift-right"), SelectEnd, Some("Composer")),
        KeyBinding::new("enter", Submit, Some("Composer")),
        KeyBinding::new("shift-enter", Newline, Some("Composer")),
        KeyBinding::new(&shortcut("v"), Paste, Some("Composer")),
        KeyBinding::new(&shortcut("x"), Cut, Some("Composer")),
        KeyBinding::new(&shortcut("c"), Copy, Some("Composer")),
        KeyBinding::new("escape", Cancel, Some("Composer")),
    ];
    if cfg!(target_os = "macos") {
        bindings.push(KeyBinding::new(
            "ctrl-cmd-space",
            ShowCharacterPalette,
            Some("Composer"),
        ));
    }
    cx.bind_keys(bindings);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerDirection {
    Previous,
    Next,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposerEvent {
    Changed(String),
    Submit(String),
    Navigate(PickerDirection),
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
    picker_navigation: bool,
    prominent: bool,
    previous_visual_lines: usize,
    visual_lines: usize,
    height_revision: u64,
}

impl Composer {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::configured("", "Message", false, true, false, false, cx)
    }

    pub fn single_line(
        text: impl Into<String>,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::configured(text, placeholder, true, false, false, false, cx)
    }

    pub fn picker(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self::configured("", placeholder, true, false, false, true, cx)
    }

    pub fn multiline(
        text: impl Into<String>,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::configured(text, placeholder, false, false, false, false, cx)
    }

    fn configured(
        text: impl Into<String>,
        placeholder: impl Into<SharedString>,
        single_line: bool,
        clear_on_submit: bool,
        read_only: bool,
        picker_navigation: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let text = text.into();
        let cursor = text.len();
        let visual_lines = estimated_visual_lines(&text);
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
            picker_navigation,
            prominent: clear_on_submit,
            previous_visual_lines: visual_lines,
            visual_lines,
            height_revision: 0,
        }
    }

    pub fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn text(&self) -> &str {
        &self.editor.text
    }

    pub fn height_transition(&self) -> (usize, usize, u64) {
        (
            self.previous_visual_lines,
            self.visual_lines,
            self.height_revision,
        )
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
        let visual_lines = estimated_visual_lines(&self.editor.text);
        if visual_lines != self.visual_lines {
            self.previous_visual_lines = self.visual_lines;
            self.visual_lines = visual_lines;
            self.height_revision = self.height_revision.wrapping_add(1);
        }
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
        if self.picker_navigation {
            cx.emit(ComposerEvent::Navigate(PickerDirection::Previous));
        } else {
            self.move_vertical(-1.0, false, cx);
        }
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if self.picker_navigation {
            cx.emit(ComposerEvent::Navigate(PickerDirection::Next));
        } else {
            self.move_vertical(1.0, false, cx);
        }
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
        if self.read_only || (!self.picker_navigation && self.editor.text.trim().is_empty()) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composer_height_estimate_is_bounded_and_tracks_wrapping() {
        assert_eq!(estimated_visual_lines(""), 1);
        assert_eq!(estimated_visual_lines("first\nsecond"), 2);
        assert_eq!(estimated_visual_lines(&"x".repeat(73)), 2);
        assert_eq!(estimated_visual_lines(&"x".repeat(1000)), 8);
    }

    #[test]
    fn input_palette_keeps_background_contrasted_with_inherited_text() {
        let dark = InputPalette::for_inherited_text(rgb(0xf3f4f6).into());
        assert_eq!(dark.background, rgba(0xffffff10));
        assert_eq!(dark.prominent_background, rgba(0x2c2c2ef7));
        assert_eq!(dark.placeholder, rgb(0x8e8e93).into());

        let light = InputPalette::for_inherited_text(rgb(0x202124).into());
        assert_eq!(light.background, rgba(0x76768014));
        assert_eq!(light.prominent_background, rgba(0xfffffffa));
        assert_eq!(light.placeholder, rgb(0x8e8e93).into());
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
