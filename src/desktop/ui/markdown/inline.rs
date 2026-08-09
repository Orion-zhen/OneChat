use std::ops::Range;

use gpui::{
    AnyElement, App, FontStyle, FontWeight, HighlightStyle, SharedString, StrikethroughStyle, div,
    prelude::*, px,
};
use gpui_component::ActiveTheme as _;
use unicode_linebreak::{BreakOpportunity, linebreaks};

use super::{InlineMetrics, next_text_index, render_formula_element, selectable};
use crate::{
    desktop::ui::{
        selectable_text::{SelectableText, TextSelection, selection_color},
        theme,
    },
    markdown::Inline,
};

pub(super) fn render_inlines(
    inlines: &[Inline],
    message_id: &str,
    text_index: &mut usize,
    selection: &TextSelection,
    scale_factor: f32,
    metrics: InlineMetrics,
    cx: &App,
) -> AnyElement {
    if inlines.iter().any(|inline| {
        matches!(inline, Inline::Formula(_))
            || matches!(inline, Inline::Text { style, .. } if style.code)
    }) {
        return render_mixed_inlines(
            inlines,
            message_id,
            text_index,
            selection,
            scale_factor,
            metrics,
            cx,
        );
    }

    let mut elements = Vec::with_capacity(inlines.len());
    let mut inline_index = 0;
    while inline_index < inlines.len() {
        match &inlines[inline_index] {
            Inline::Text { style, .. } if !style.code => {
                let (text, next_index) = collect_text_run(inlines, inline_index);
                let highlights = text_highlights(&text.styles, 0..text.content.len(), cx);
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
                                cx,
                            )
                            .with_highlights(highlights),
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
                        .when(style.emphasis, |element| element.italic())
                        .when(style.strong, |element| {
                            element.font_weight(FontWeight::BOLD)
                        })
                        .when(style.strike, |element| element.line_through())
                        .rounded_md()
                        .bg(cx.theme().muted)
                        .px_1()
                        .font(theme::code_font(cx))
                        .text_size(px(metrics.code_size()))
                        .when(style.link, |element| element.text_color(cx.theme().primary))
                        .child(selectable(
                            message_id,
                            next_text_index(text_index),
                            content.clone(),
                            selection,
                            cx,
                        ))
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
    message_id: &str,
    text_index: &mut usize,
    selection: &TextSelection,
    scale_factor: f32,
    metrics: InlineMetrics,
    cx: &App,
) -> AnyElement {
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
                        content, *style, message_id, text_index, selection, metrics, cx,
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
                    let highlights = text_highlights(&text.styles, source_range.clone(), cx);
                    unit.push(
                        SelectableText::fragment(
                            text.id.clone(),
                            text.source.clone(),
                            source_range,
                            selection.clone(),
                            selection_color(cx.theme().is_dark()),
                        )
                        .with_highlights(highlights)
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
    message_id: &str,
    text_index: &mut usize,
    selection: &TextSelection,
    metrics: InlineMetrics,
    cx: &App,
) -> AnyElement {
    div()
        .flex_none()
        .whitespace_nowrap()
        .when(style.emphasis, |element| element.italic())
        .when(style.strong, |element| {
            element.font_weight(FontWeight::BOLD)
        })
        .when(style.strike, |element| element.line_through())
        .rounded_md()
        .bg(cx.theme().muted)
        .px_1()
        .font(theme::code_font(cx))
        .text_size(px(metrics.code_size()))
        .when(style.link, |element| element.text_color(cx.theme().primary))
        .child(selectable(
            message_id,
            next_text_index(text_index),
            content.to_string(),
            selection,
            cx,
        ))
        .into_any_element()
}

fn text_highlights(
    styles: &[(Range<usize>, crate::markdown::InlineStyle)],
    source_range: Range<usize>,
    cx: &App,
) -> Vec<(Range<usize>, HighlightStyle)> {
    styles
        .iter()
        .filter_map(|(range, style)| {
            let start = range.start.max(source_range.start);
            let end = range.end.min(source_range.end);
            (start < end).then(|| {
                (
                    (start - source_range.start)..(end - source_range.start),
                    text_highlight(*style, cx),
                )
            })
        })
        .filter(|(_, highlight)| *highlight != HighlightStyle::default())
        .collect()
}

fn text_highlight(style: crate::markdown::InlineStyle, cx: &App) -> HighlightStyle {
    HighlightStyle {
        color: style.link.then_some(cx.theme().primary),
        font_weight: style.strong.then_some(FontWeight::BOLD),
        font_style: style.emphasis.then_some(FontStyle::Italic),
        strikethrough: style.strike.then_some(StrikethroughStyle {
            thickness: px(1.0),
            ..Default::default()
        }),
        ..Default::default()
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
