use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use gpui::{
    AnyElement, App, FontWeight, Image, ImageFormat, SharedString, div, img, prelude::*, px,
};
use gpui_component::{ActiveTheme as _, ThemeMode};

use super::selectable_text::{SelectableText, TextSelection, selection_color};
use crate::markdown::{
    Block, Formula, Inline, MarkdownDocument, TableAlignment, render_formula_cached,
};

pub(crate) fn render(
    document: &MarkdownDocument,
    message_id: &str,
    selection: &TextSelection,
    scale_factor: f32,
    cx: &App,
) -> AnyElement {
    let mut text_index = 0;
    render_blocks(
        &document.blocks,
        message_id,
        &mut text_index,
        selection,
        scale_factor,
        cx,
    )
}

pub(crate) fn render_plain(
    source: &str,
    message_id: &str,
    selection: &TextSelection,
    cx: &App,
) -> AnyElement {
    div()
        .whitespace_normal()
        .line_height(px(24.0))
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
    cx: &App,
) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        .children(
            blocks.iter().map(|block| {
                render_block(block, message_id, text_index, selection, scale_factor, cx)
            }),
        )
        .into_any_element()
}

fn render_block(
    block: &Block,
    message_id: &str,
    text_index: &mut usize,
    selection: &TextSelection,
    scale_factor: f32,
    cx: &App,
) -> AnyElement {
    match block {
        Block::Paragraph(inlines) => render_inlines(
            inlines,
            message_id,
            text_index,
            selection,
            scale_factor,
            cx,
            false,
        ),
        Block::Heading(level, inlines) => div()
            .pt(if *level <= 2 { px(8.0) } else { px(4.0) })
            .text_size(match level {
                1 => px(24.0),
                2 => px(21.0),
                3 => px(18.0),
                _ => px(16.0),
            })
            .font_weight(FontWeight::SEMIBOLD)
            .child(render_inlines(
                inlines,
                message_id,
                text_index,
                selection,
                scale_factor,
                cx,
                false,
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
                        cx,
                    )))
            }))
            .into_any_element(),
        Block::Code { language, content } => div()
            .w_full()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(selectable(
                        message_id,
                        next_text_index(text_index),
                        if language.is_empty() {
                            "Code".into()
                        } else {
                            language.clone()
                        },
                        selection,
                        cx,
                    )),
            )
            .child(
                div()
                    .id(element_key("code", content))
                    .w_full()
                    .overflow_scroll()
                    .p_3()
                    .font_family("SFMono-Regular")
                    .text_sm()
                    .whitespace_nowrap()
                    .child(selectable(
                        message_id,
                        next_text_index(text_index),
                        content.clone(),
                        selection,
                        cx,
                    )),
            )
            .into_any_element(),
        Block::Formula(formula) => render_formula_element(formula, scale_factor, cx),
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
    cx: &App,
    table_cell: bool,
) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_wrap()
        .items_baseline()
        .line_height(px(if table_cell { 21.0 } else { 24.0 }))
        .children(inlines.iter().map(|inline| match inline {
            Inline::Text { content, style } => {
                let content = content.clone();
                div()
                    .max_w_full()
                    .whitespace_normal()
                    .when(style.emphasis, |element| element.italic())
                    .when(style.strong, |element| {
                        element.font_weight(FontWeight::BOLD)
                    })
                    .when(style.strike, |element| element.line_through())
                    .when(style.code, |element| {
                        element
                            .rounded_md()
                            .bg(cx.theme().muted)
                            .px_1()
                            .font_family("SFMono-Regular")
                            .text_sm()
                    })
                    .when(style.link, |element| element.text_color(cx.theme().primary))
                    .child(selectable(
                        message_id,
                        next_text_index(text_index),
                        content,
                        selection,
                        cx,
                    ))
                    .into_any_element()
            }
            Inline::Formula(formula) => render_formula_element(formula, scale_factor, cx),
            Inline::Break => div().w_full().h(px(1.0)).into_any_element(),
        }))
        .into_any_element()
}

fn render_formula_element(formula: &Formula, scale_factor: f32, cx: &App) -> AnyElement {
    match render_formula_cached(
        &formula.source,
        formula.display,
        cx.theme().mode == ThemeMode::Dark,
        scale_factor,
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
            .text_sm()
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
                            cx,
                            true,
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
