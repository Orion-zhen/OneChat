use super::*;

pub(super) struct TextElement {
    pub(super) input: Entity<Composer>,
}

#[derive(Clone, Default)]
pub(super) struct RequestedLayout(Rc<RefCell<Option<InputLayout>>>);

pub(super) struct PrepaintState {
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
