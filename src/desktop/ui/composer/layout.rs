use super::*;

pub(super) fn estimated_visual_lines(text: &str) -> usize {
    text.split('\n')
        .map(|line| line.chars().count().div_ceil(72).max(1))
        .sum::<usize>()
        .clamp(1, 8)
}

pub(super) fn text_runs(base: TextRun, marked_range: Option<Range<usize>>) -> Vec<TextRun> {
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
pub(super) struct InputLayout {
    pub(super) lines: Vec<InputLine>,
    pub(super) line_height: Pixels,
    pub(super) height: Pixels,
    content_len: usize,
}

#[derive(Clone)]
pub(super) struct InputLine {
    pub(super) line: WrappedLine,
    pub(super) range: Range<usize>,
    pub(super) y: Pixels,
    pub(super) height: Pixels,
}

impl InputLayout {
    pub(super) fn empty(line_height: Pixels, content_len: usize) -> Self {
        Self {
            lines: Vec::new(),
            line_height,
            height: line_height,
            content_len,
        }
    }

    pub(super) fn new(
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

    pub(super) fn position_for_index(&self, index: usize) -> Point<Pixels> {
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

    pub(super) fn index_for_position(&self, position: Point<Pixels>) -> usize {
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

    pub(super) fn selection_quads(
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
