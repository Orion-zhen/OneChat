use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    ops::Range,
};

use gpui::{
    AnyElement, App, ClipboardItem, FontStyle, FontWeight, HighlightStyle, Image, ImageFormat,
    SharedString, StrikethroughStyle, div, img, prelude::*, px, rgba,
};
use gpui_component::{
    ActiveTheme as _, ThemeMode,
    button::{Button, ButtonVariants as _},
};
use unicode_linebreak::{BreakOpportunity, linebreaks};

use super::icons::{AppIcon, IconTone, render_icon};
use super::selectable_text::{SelectableText, TextSelection, selection_color};
use super::typography::MessageTypography;
use crate::markdown::{
    Block, Formula, Inline, MarkdownDocument, TableAlignment, render_formula_cached,
};

#[derive(Clone, Copy)]
struct InlineMetrics {
    size: f32,
    line_height: f32,
}

impl InlineMetrics {
    fn new(size: f32, line_height: f32) -> Self {
        Self { size, line_height }
    }

    fn code_size(self) -> f32 {
        self.size - 1.0
    }

    fn code_line_height(self) -> f32 {
        self.code_size() + 8.0
    }

    fn formula_scale(self) -> f32 {
        self.size / crate::domain::DEFAULT_MESSAGE_FONT_SIZE
    }
}

pub(crate) fn render(
    document: &MarkdownDocument,
    message_id: &str,
    selection: &TextSelection,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &App,
) -> AnyElement {
    let mut text_index = 0;
    render_blocks(
        &document.blocks,
        message_id,
        &mut text_index,
        selection,
        scale_factor,
        typography,
        cx,
    )
}

pub(crate) fn render_plain(
    source: &str,
    message_id: &str,
    selection: &TextSelection,
    typography: MessageTypography,
    cx: &App,
) -> AnyElement {
    div()
        .whitespace_normal()
        .text_size(px(typography.body_size))
        .line_height(px(typography.body_line_height))
        .child(selectable(message_id, 0, source.to_string(), selection, cx))
        .text_color(cx.theme().foreground)
        .into_any_element()
}

fn render_blocks(
    blocks: &[Block],
    message_id: &str,
    text_index: &mut usize,
    selection: &TextSelection,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &App,
) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        .text_size(px(typography.body_size))
        .line_height(px(typography.body_line_height))
        .children(blocks.iter().map(|block| {
            render_block(
                block,
                message_id,
                text_index,
                selection,
                scale_factor,
                typography,
                cx,
            )
        }))
        .into_any_element()
}

fn render_block(
    block: &Block,
    message_id: &str,
    text_index: &mut usize,
    selection: &TextSelection,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &App,
) -> AnyElement {
    match block {
        Block::Paragraph(inlines) => render_inlines(
            inlines,
            message_id,
            text_index,
            selection,
            scale_factor,
            InlineMetrics::new(typography.body_size, typography.body_line_height),
            cx,
        ),
        Block::Heading(level, inlines) => div()
            .pt(if *level <= 2 { px(8.0) } else { px(4.0) })
            .text_size(px(typography.heading_size(*level)))
            .line_height(px(typography.heading_line_height(*level)))
            .font_weight(FontWeight::SEMIBOLD)
            .child(render_inlines(
                inlines,
                message_id,
                text_index,
                selection,
                scale_factor,
                InlineMetrics::new(
                    typography.heading_size(*level),
                    typography.heading_line_height(*level),
                ),
                cx,
            ))
            .into_any_element(),
        Block::Quote(blocks) => div()
            .w_full()
            .border_l_2()
            .border_color(cx.theme().primary)
            .pl_4()
            .py_1()
            .text_color(cx.theme().muted_foreground)
            .child(render_blocks(
                blocks,
                message_id,
                text_index,
                selection,
                scale_factor,
                typography,
                cx,
            ))
            .into_any_element(),
        Block::List {
            ordered,
            start,
            items,
        } => div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .children(items.iter().enumerate().map(|(index, blocks)| {
                div()
                    .w_full()
                    .flex()
                    .items_start()
                    .gap_2()
                    .child(
                        div()
                            .w(px(24.0))
                            .flex_none()
                            .text_color(cx.theme().muted_foreground)
                            .child(if *ordered {
                                format!("{}.", start + index)
                            } else {
                                "•".into()
                            }),
                    )
                    .child(div().min_w_0().flex_1().child(render_blocks(
                        blocks,
                        message_id,
                        text_index,
                        selection,
                        scale_factor,
                        typography,
                        cx,
                    )))
            }))
            .into_any_element(),
        Block::Code {
            language,
            content,
            highlights,
        } => {
            let language_text_index = next_text_index(text_index);
            let content_text_index = next_text_index(text_index);
            let copy_button_id =
                SharedString::from(format!("copy-code-block-{message_id}-{content_text_index}"));
            let content_to_copy = content.clone();

            div()
                .w_full()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().muted)
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .pl_3()
                        .pr_1()
                        .py_1()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .text_size(px(typography.micro_size))
                        .line_height(px(typography.micro_line_height))
                        .text_color(cx.theme().muted_foreground)
                        .child(div().min_w_0().flex_1().child(selectable(
                            message_id,
                            language_text_index,
                            if language.is_empty() {
                                "Code".into()
                            } else {
                                language.clone()
                            },
                            selection,
                            cx,
                        )))
                        .child(
                            Button::new(copy_button_id)
                                .ghost()
                                .tooltip("Copy code")
                                .size(px(28.0))
                                .p_0()
                                .child(render_icon(AppIcon::Copy, IconTone::Muted, 16.0, cx))
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        content_to_copy.clone(),
                                    ));
                                }),
                        ),
                )
                .child(
                    div()
                        .id(element_key("code", content))
                        .w_full()
                        .overflow_scroll()
                        .p_3()
                        .font(super::theme::code_font(cx))
                        .text_size(px(typography.code_size))
                        .line_height(px(typography.code_line_height))
                        .whitespace_nowrap()
                        .child(
                            selectable(
                                message_id,
                                content_text_index,
                                content.clone(),
                                selection,
                                cx,
                            )
                            .with_highlights(
                                if cx.theme().is_dark() {
                                    &highlights.dark
                                } else {
                                    &highlights.light
                                }
                                .iter()
                                .map(|highlight| {
                                    (
                                        highlight.range.clone(),
                                        HighlightStyle::from(rgba(highlight.rgba)),
                                    )
                                }),
                            ),
                        ),
                )
                .into_any_element()
        }
        Block::Formula(formula) => render_formula_element(
            formula,
            scale_factor,
            InlineMetrics::new(typography.body_size, typography.body_line_height),
            cx,
        ),
        Block::Table {
            alignments,
            header,
            rows,
        } => render_table(
            TableContent {
                alignments,
                header,
                rows,
            },
            message_id,
            text_index,
            selection,
            scale_factor,
            typography,
            cx,
        ),
        Block::Rule => div()
            .w_full()
            .h(px(1.0))
            .my_2()
            .bg(cx.theme().border)
            .into_any_element(),
    }
}

fn render_inlines(
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
                        .font(super::theme::code_font(cx))
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
        .font(super::theme::code_font(cx))
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

fn render_formula_element(
    formula: &Formula,
    scale_factor: f32,
    metrics: InlineMetrics,
    cx: &App,
) -> AnyElement {
    match render_formula_cached(
        &formula.source,
        formula.display,
        cx.theme().mode == ThemeMode::Dark,
        scale_factor,
        metrics.formula_scale(),
    ) {
        Ok(rendered) => {
            let image =
                std::sync::Arc::new(Image::from_bytes(ImageFormat::Svg, rendered.svg.clone()));
            div()
                .id(element_key("formula", &formula.source))
                .when(formula.display, |element| {
                    element.w_full().justify_center().overflow_scroll().py_2()
                })
                .when(!formula.display, |element| element.px_1())
                .flex()
                .items_center()
                .child(
                    img(image)
                        .w(px(rendered.width))
                        .h(px(rendered.height))
                        .flex_none(),
                )
                .into_any_element()
        }
        Err(error) => div()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().danger)
            .bg(cx.theme().muted)
            .px_2()
            .py_1()
            .font(super::theme::code_font(cx))
            .text_size(px(metrics.code_size()))
            .line_height(px(metrics.code_line_height()))
            .text_color(cx.theme().danger)
            .child(format!("{} · {error}", formula.source))
            .into_any_element(),
    }
}

struct TableContent<'a> {
    alignments: &'a [TableAlignment],
    header: &'a [Vec<Inline>],
    rows: &'a [Vec<Vec<Inline>>],
}

fn render_table(
    table: TableContent<'_>,
    message_id: &str,
    text_index: &mut usize,
    selection: &TextSelection,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &App,
) -> AnyElement {
    let columns = table
        .header
        .len()
        .max(table.rows.iter().map(Vec::len).max().unwrap_or_default());
    let mut render_row = |cells: &[Vec<Inline>], header_row: bool| {
        div()
            .min_w(px(columns.max(1) as f32 * 130.0))
            .flex()
            .children((0..columns).map(|index| {
                let alignment = table
                    .alignments
                    .get(index)
                    .copied()
                    .unwrap_or(TableAlignment::None);
                div()
                    .min_w(px(130.0))
                    .flex_1()
                    .border_r_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .p_2()
                    .when(header_row, |element| {
                        element
                            .bg(cx.theme().muted)
                            .font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(alignment == TableAlignment::Center, |element| {
                        element.text_center()
                    })
                    .when(alignment == TableAlignment::Right, |element| {
                        element.text_right()
                    })
                    .children(cells.get(index).map(|cell| {
                        render_inlines(
                            cell,
                            message_id,
                            text_index,
                            selection,
                            scale_factor,
                            InlineMetrics::new(
                                typography.body_size,
                                typography.table_line_height(),
                            ),
                            cx,
                        )
                    }))
            }))
    };

    div()
        .id(element_key(
            "table",
            table
                .header
                .first()
                .and_then(|cell| cell.first())
                .and_then(|inline| match inline {
                    Inline::Text { content, .. } => Some(content.as_str()),
                    _ => None,
                })
                .unwrap_or("empty"),
        ))
        .w_full()
        .overflow_scroll()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .flex_col()
                .children((!table.header.is_empty()).then(|| render_row(table.header, true)))
                .children(table.rows.iter().map(|row| render_row(row, false))),
        )
        .into_any_element()
}

fn next_text_index(index: &mut usize) -> usize {
    let current = *index;
    *index += 1;
    current
}

fn selectable(
    message_id: &str,
    index: usize,
    content: String,
    selection: &TextSelection,
    cx: &App,
) -> SelectableText {
    SelectableText::new(
        SharedString::from(format!("message-text-{message_id}-{index}")),
        content,
        selection.clone(),
        selection_color(cx.theme().is_dark()),
    )
}

fn element_key(prefix: &str, content: &str) -> SharedString {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("markdown-{prefix}-{:x}", hasher.finish()).into()
}
