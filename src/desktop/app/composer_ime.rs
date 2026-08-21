use std::ops::Range;

#[cfg(not(target_os = "linux"))]
use gpui::IntoElement as _;
use gpui::{
    AnyElement, App, Bounds, Context, Entity, EntityInputHandler, Pixels, Point, UTF16Selection,
    Window,
};
#[cfg(target_os = "linux")]
use gpui::{ElementInputHandler, Focusable, canvas, prelude::*};
use gpui_component::input::TextareaState;

/// Keeps the platform IME candidate anchor stable while the textarea is composing.
///
/// gpui-component lays out marked text on the next frame, but Wayland asks for its bounds
/// immediately. Forwarding every queried marked-text range to the previous frame's layout can
/// therefore alternate the candidate window between the caret and the textarea origin. This
/// handler delegates editing to TextareaState while answering all geometry queries in one
/// composition with the bounds captured at the marked range's start.
pub(crate) struct ComposerImeHandler {
    textarea: Entity<TextareaState>,
    composition_anchor: Option<Bounds<Pixels>>,
}

impl ComposerImeHandler {
    pub(crate) fn new(textarea: Entity<TextareaState>) -> Self {
        Self {
            textarea,
            composition_anchor: None,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.composition_anchor = None;
    }

    fn marked_range(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<Range<usize>> {
        self.textarea.update(cx, |textarea, cx| {
            EntityInputHandler::marked_text_range(textarea, window, cx)
        })
    }

    fn element_bounds(&self, cx: &App) -> Option<Bounds<Pixels>> {
        self.textarea.read(cx).text_bounds()
    }
}

impl EntityInputHandler for ComposerImeHandler {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        self.textarea.update(cx, |textarea, cx| {
            EntityInputHandler::text_for_range(textarea, range, adjusted_range, window, cx)
        })
    }

    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        self.textarea.update(cx, |textarea, cx| {
            EntityInputHandler::selected_text_range(textarea, ignore_disabled_input, window, cx)
        })
    }

    fn marked_text_range(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range(window, cx)
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.composition_anchor = None;
        self.textarea.update(cx, |textarea, cx| {
            EntityInputHandler::unmark_text(textarea, window, cx)
        });
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.composition_anchor = None;
        self.textarea.update(cx, |textarea, cx| {
            EntityInputHandler::replace_text_in_range(textarea, range, text, window, cx)
        });
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.marked_range(window, cx).is_none() {
            self.composition_anchor = None;
        }
        self.textarea.update(cx, |textarea, cx| {
            EntityInputHandler::replace_and_mark_text_in_range(
                textarea,
                range,
                new_text,
                new_selected_range,
                window,
                cx,
            )
        });
        if new_text.is_empty() {
            self.composition_anchor = None;
        }
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        element_bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let bounds = self.element_bounds(cx).unwrap_or(element_bounds);
        if let Some(marked) = self.marked_range(window, cx) {
            if let Some(anchor) = self.composition_anchor {
                return Some(anchor);
            }

            let anchor = self.textarea.update(cx, |textarea, cx| {
                EntityInputHandler::bounds_for_range(
                    textarea,
                    marked.start..marked.start,
                    bounds,
                    window,
                    cx,
                )
            });
            self.composition_anchor = anchor;
            return anchor;
        }

        self.composition_anchor = None;
        self.textarea.update(cx, |textarea, cx| {
            EntityInputHandler::bounds_for_range(textarea, range, bounds, window, cx)
        })
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        self.textarea.update(cx, |textarea, cx| {
            EntityInputHandler::character_index_for_point(textarea, point, window, cx)
        })
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.textarea.update(cx, |textarea, cx| {
            EntityInputHandler::set_selected_text_range(textarea, range, window, cx)
        });
    }

    fn text_length_utf16(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<usize> {
        self.textarea.update(cx, |textarea, cx| {
            EntityInputHandler::text_length_utf16(textarea, window, cx)
        })
    }

    fn accepts_text_input(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.textarea.update(cx, |textarea, cx| {
            EntityInputHandler::accepts_text_input(textarea, window, cx)
        })
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn register_composer_ime(handler: &Entity<ComposerImeHandler>) -> AnyElement {
    let handler = handler.clone();
    canvas(
        |_, _, _| {},
        move |_, _, window, cx| {
            let textarea = handler.read(cx).textarea.clone();
            let textarea = textarea.read(cx);
            let Some(bounds) = textarea.text_bounds() else {
                return;
            };
            let focus = textarea.focus_handle(cx);
            window.handle_input(
                &focus,
                ElementInputHandler::new(bounds, handler.clone()),
                cx,
            );
        },
    )
    .absolute()
    .inset_0()
    .into_any_element()
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn register_composer_ime(_: &Entity<ComposerImeHandler>) -> AnyElement {
    gpui::div().into_any_element()
}
