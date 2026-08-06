use super::*;

#[derive(Debug, Default)]
pub(super) struct EditorState {
    pub(super) text: String,
    pub(super) selection: Range<usize>,
    pub(super) selection_reversed: bool,
}

impl EditorState {
    pub(super) fn cursor(&self) -> usize {
        if self.selection_reversed {
            self.selection.start
        } else {
            self.selection.end
        }
    }

    pub(super) fn move_to(&mut self, offset: usize) {
        let offset = self.clamp_boundary(offset);
        self.selection = offset..offset;
        self.selection_reversed = false;
    }

    pub(super) fn select_to(&mut self, offset: usize) {
        let offset = self.clamp_boundary(offset);
        let anchor = if self.selection_reversed {
            self.selection.end
        } else {
            self.selection.start
        };
        self.selection = anchor.min(offset)..anchor.max(offset);
        self.selection_reversed = offset < anchor;
    }

    pub(super) fn replace_selection(&mut self, new_text: &str) {
        self.replace_range(self.selection.clone(), new_text);
    }

    pub(super) fn replace_range(&mut self, range: Range<usize>, new_text: &str) {
        let range = self.clamp_boundary(range.start)..self.clamp_boundary(range.end);
        self.text.replace_range(range.clone(), new_text);
        self.move_to(range.start + new_text.len());
    }

    pub(super) fn previous_boundary(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    pub(super) fn next_boundary(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.text.len())
    }

    pub(super) fn line_start(&self) -> usize {
        let cursor = self.cursor();
        self.text[..cursor].rfind('\n').map_or(0, |index| index + 1)
    }

    pub(super) fn line_end(&self) -> usize {
        let cursor = self.cursor();
        self.text[cursor..]
            .find('\n')
            .map_or(self.text.len(), |index| cursor + index)
    }

    pub(super) fn clamp_boundary(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    pub(super) fn offset_from_utf16(&self, offset: usize) -> usize {
        offset_from_utf16_in(&self.text, offset)
    }

    pub(super) fn offset_to_utf16(&self, offset: usize) -> usize {
        self.text[..self.clamp_boundary(offset)]
            .encode_utf16()
            .count()
    }

    pub(super) fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    pub(super) fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }
}

pub(super) fn offset_from_utf16_in(text: &str, offset: usize) -> usize {
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

pub(super) fn range_from_utf16_in(text: &str, range: &Range<usize>) -> Range<usize> {
    offset_from_utf16_in(text, range.start)..offset_from_utf16_in(text, range.end)
}
