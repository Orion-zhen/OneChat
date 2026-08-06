use super::*;

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
