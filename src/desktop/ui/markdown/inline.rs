use std::ops::Range;

use gpui::{
    AnyElement, FontStyle, FontWeight, HighlightStyle, SharedString, StrikethroughStyle,
    UnderlineStyle, div, prelude::*, px,
};
use unicode_linebreak::{BreakOpportunity, linebreaks};

use super::{
    InlineMetrics, MarkdownContext, MarkdownPalette, next_text_index, render_formula_element,
    selectable,
};
use crate::{
    desktop::ui::{
        selectable_text::{AdaptiveHighlight, SelectableText},
        theme,
    },
    markdown::Inline,
};

pub(super) fn render_inlines(
    inlines: &[Inline],
    text_index: &mut usize,
    metrics: InlineMetrics,
    context: &MarkdownContext<'_>,
) -> AnyElement {
    let message_id = context.message_id;
    let selection = context.selection;
    let scale_factor = context.scale_factor;
    let palette = context.palette;
    let cx = context.cx;

    if inlines.iter().any(|inline| {
        matches!(inline, Inline::Formula(_))
            || matches!(inline, Inline::Text { style, .. } if style.code)
    }) {
        return render_mixed_inlines(inlines, text_index, metrics, context);
    }

    let mut elements = Vec::with_capacity(inlines.len());
    let mut inline_index = 0;
    while inline_index < inlines.len() {
        match &inlines[inline_index] {
            Inline::Text { style, .. } if !style.code => {
                let (text, next_index) = collect_text_run(inlines, inline_index);
                let highlights = text_highlights(&text.styles, 0..text.content.len(), palette);
                elements.push(
                    div()
                        .max_w_full()
                        .whitespace_normal()
                        .child(
                            selectable(
                                message_id,
                                next_text_index(text_index),
                                text.content,
                                selection,
                                palette,
                            )
                            .with_adaptive_highlights(highlights),
                        )
                        .into_any_element(),
                );
                inline_index = next_index;
            }
            Inline::Text { content, style } => {
                elements.push(
                    div()
                        .max_w_full()
                        .whitespace_normal()
                        .rounded_md()
                        .bg(palette.surface)
                        .px_1()
                        .font(theme::code_font(cx))
                        .text_size(px(metrics.code_size()))
                        .child(
                            selectable(
                                message_id,
                                next_text_index(text_index),
                                content.clone(),
                                selection,
                                palette,
                            )
                            .with_adaptive_highlights([
                                adaptive_highlight(
                                    0..content.len(),
                                    0..content.len(),
                                    *style,
                                    palette,
                                ),
                            ]),
                        )
                        .into_any_element(),
                );
                inline_index += 1;
            }
            Inline::Formula(formula) => {
                elements.push(render_formula_element(formula, scale_factor, metrics, cx));
                inline_index += 1;
            }
            Inline::Break => {
                elements.push(div().w_full().h(px(1.0)).into_any_element());
                inline_index += 1;
            }
        }
    }

    div()
        .w_full()
        .flex()
        .flex_wrap()
        .items_baseline()
        .text_size(px(metrics.size))
        .line_height(px(metrics.line_height))
        .children(elements)
        .into_any_element()
}

const INLINE_ATOM: char = '\u{fffc}';

struct FlowText {
    virtual_range: Range<usize>,
    id: SharedString,
    source: SharedString,
    styles: Vec<(Range<usize>, crate::markdown::InlineStyle)>,
}

struct FlowAtom {
    virtual_range: Range<usize>,
    element: Option<AnyElement>,
}

enum FlowPart {
    Text(FlowText),
    Atom(FlowAtom),
    Break(Range<usize>),
}

impl FlowPart {
    fn virtual_range(&self) -> &Range<usize> {
        match self {
            Self::Text(text) => &text.virtual_range,
            Self::Atom(atom) => &atom.virtual_range,
            Self::Break(range) => range,
        }
    }
}

fn render_mixed_inlines(
    inlines: &[Inline],
    text_index: &mut usize,
    metrics: InlineMetrics,
    context: &MarkdownContext<'_>,
) -> AnyElement {
    let message_id = context.message_id;
    let selection = context.selection;
    let scale_factor = context.scale_factor;
    let palette = context.palette;
    let cx = context.cx;

    let mut virtual_source = String::new();
    let mut parts = Vec::with_capacity(inlines.len());
    let mut inline_index = 0;

    while inline_index < inlines.len() {
        match &inlines[inline_index] {
            Inline::Text { style, .. } if !style.code => {
                let (text, next_index) = collect_text_run(inlines, inline_index);
                let start = virtual_source.len();
                virtual_source.push_str(&text.content);
                parts.push(FlowPart::Text(FlowText {
                    virtual_range: start..virtual_source.len(),
                    id: format!("message-text-{message_id}-{}", next_text_index(text_index)).into(),
                    source: text.content.into(),
                    styles: text.styles,
                }));
                inline_index = next_index;
            }
            Inline::Text { content, style } => {
                let start = virtual_source.len();
                virtual_source.push(INLINE_ATOM);
                parts.push(FlowPart::Atom(FlowAtom {
                    virtual_range: start..virtual_source.len(),
                    element: Some(render_inline_code(
                        content, *style, text_index, metrics, context,
                    )),
                }));
                inline_index += 1;
            }
            Inline::Formula(formula) => {
                let start = virtual_source.len();
                virtual_source.push(INLINE_ATOM);
                parts.push(FlowPart::Atom(FlowAtom {
                    virtual_range: start..virtual_source.len(),
                    element: Some(render_formula_element(formula, scale_factor, metrics, cx)),
                }));
                inline_index += 1;
            }
            Inline::Break => {
                let start = virtual_source.len();
                virtual_source.push('\n');
                parts.push(FlowPart::Break(start..virtual_source.len()));
                inline_index += 1;
            }
        }
    }

    let mut elements = Vec::new();
    let mut unit_start = 0;
    for (unit_end, opportunity) in linebreaks(&virtual_source) {
        let unit_range = unit_start..unit_end;
        let mut unit = Vec::new();

        for part in &mut parts {
            let part_range = part.virtual_range();
            let start = unit_range.start.max(part_range.start);
            let end = unit_range.end.min(part_range.end);
            if start >= end {
                continue;
            }

            match part {
                FlowPart::Text(text) => {
                    let source_range =
                        (start - text.virtual_range.start)..(end - text.virtual_range.start);
                    let highlights = text_highlights(&text.styles, source_range.clone(), palette);
                    unit.push(
                        SelectableText::fragment(
                            text.id.clone(),
                            text.source.clone(),
                            source_range,
                            selection.clone(),
                            palette.selection,
                        )
                        .with_adaptive_highlights(highlights)
                        .into_any_element(),
                    );
                }
                FlowPart::Atom(atom) => {
                    if let Some(element) = atom.element.take() {
                        unit.push(element);
                    }
                }
                FlowPart::Break(_) => {}
            }
        }

        if !unit.is_empty() {
            elements.push(
                div()
                    .flex()
                    .flex_none()
                    .min_h(px(metrics.line_height))
                    .items_center()
                    .whitespace_nowrap()
                    .children(unit)
                    .into_any_element(),
            );
        }
        if opportunity == BreakOpportunity::Mandatory && virtual_source[..unit_end].ends_with('\n')
        {
            elements.push(div().w_full().h(px(1.0)).into_any_element());
        }
        unit_start = unit_end;
    }

    div()
        .w_full()
        .flex()
        .flex_wrap()
        .items_center()
        .text_size(px(metrics.size))
        .line_height(px(metrics.line_height))
        .children(elements)
        .into_any_element()
}

fn render_inline_code(
    content: &str,
    style: crate::markdown::InlineStyle,
    text_index: &mut usize,
    metrics: InlineMetrics,
    context: &MarkdownContext<'_>,
) -> AnyElement {
    let message_id = context.message_id;
    let selection = context.selection;
    let palette = context.palette;
    let cx = context.cx;

    div()
        .flex_none()
        .whitespace_nowrap()
        .rounded_md()
        .bg(palette.surface)
        .px_1()
        .font(theme::code_font(cx))
        .text_size(px(metrics.code_size()))
        .child(
            selectable(
                message_id,
                next_text_index(text_index),
                content.to_string(),
                selection,
                palette,
            )
            .with_adaptive_highlights([adaptive_highlight(
                0..content.len(),
                0..content.len(),
                style,
                palette,
            )]),
        )
        .into_any_element()
}

fn text_highlights(
    styles: &[(Range<usize>, crate::markdown::InlineStyle)],
    source_range: Range<usize>,
    palette: MarkdownPalette,
) -> Vec<AdaptiveHighlight> {
    styles
        .iter()
        .filter_map(|(range, style)| {
            let start = range.start.max(source_range.start);
            let end = range.end.min(source_range.end);
            (start < end).then(|| {
                adaptive_highlight(
                    (start - source_range.start)..(end - source_range.start),
                    range.clone(),
                    *style,
                    palette,
                )
            })
        })
        .filter(|highlight| highlight.style != HighlightStyle::default())
        .collect()
}

fn adaptive_highlight(
    range: Range<usize>,
    variant_range: Range<usize>,
    style: crate::markdown::InlineStyle,
    palette: MarkdownPalette,
) -> AdaptiveHighlight {
    AdaptiveHighlight {
        range,
        variant_range,
        style: HighlightStyle {
            color: style.link.then_some(palette.accent),
            font_weight: style.strong.then_some(FontWeight::BOLD),
            font_style: style.emphasis.then_some(FontStyle::Italic),
            strikethrough: style.strike.then_some(StrikethroughStyle {
                thickness: px(1.0),
                ..Default::default()
            }),
            ..Default::default()
        },
        missing_weight: style.strong.then_some(HighlightStyle {
            color: (!style.link).then_some(palette.soft_emphasis),
            underline: style.link.then_some(UnderlineStyle {
                thickness: px(2.0),
                color: Some(palette.accent),
                ..Default::default()
            }),
            ..Default::default()
        }),
        missing_style: style.emphasis.then_some(HighlightStyle {
            underline: Some(UnderlineStyle {
                thickness: px(1.0),
                ..Default::default()
            }),
            ..Default::default()
        }),
    }
}

#[derive(Debug, Default)]
struct InlineTextRun {
    content: String,
    styles: Vec<(Range<usize>, crate::markdown::InlineStyle)>,
}

fn collect_text_run(inlines: &[Inline], start: usize) -> (InlineTextRun, usize) {
    let mut run = InlineTextRun::default();
    let mut index = start;
    while let Some(Inline::Text { content, style }) = inlines.get(index)
        && !style.code
    {
        let range_start = run.content.len();
        run.content.push_str(content);
        run.styles.push((range_start..run.content.len(), *style));
        index += 1;
    }
    (run, index)
}
