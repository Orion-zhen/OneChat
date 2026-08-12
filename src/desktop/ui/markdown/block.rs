use gpui::{AnyElement, FontWeight, HighlightStyle, SharedString, div, prelude::*, px, rgba};
use gpui_component::ActiveTheme as _;

use super::{
    InlineMetrics, MarkdownContext, element_key, next_text_index, render_blocks,
    render_formula_element, render_inlines, selectable,
};
use crate::{
    desktop::ui::{copy_button::CopyButton, stream::nested_horizontal_scroll_captures, theme},
    markdown::{Block, Inline, TableAlignment},
};

struct TableContent<'a> {
    alignments: &'a [TableAlignment],
    header: &'a [Vec<Inline>],
    rows: &'a [Vec<Vec<Inline>>],
}

pub(super) fn render_block(
    block: &Block,
    text_index: &mut usize,
    context: &MarkdownContext<'_>,
) -> AnyElement {
    let message_id = context.message_id;
    let selection = context.selection;
    let scale_factor = context.scale_factor;
    let typography = context.typography;
    let palette = context.palette;
    let cx = context.cx;

    match block {
        Block::Paragraph(inlines) => render_inlines(
            inlines,
            text_index,
            InlineMetrics::new(typography.body_size, typography.body_line_height),
            context,
        ),
        Block::Heading(level, inlines) => div()
            .pt(if *level <= 2 { px(8.0) } else { px(4.0) })
            .text_size(px(typography.heading_size(*level)))
            .line_height(px(typography.heading_line_height(*level)))
            .font_weight(FontWeight::SEMIBOLD)
            .child(render_inlines(
                inlines,
                text_index,
                InlineMetrics::new(
                    typography.heading_size(*level),
                    typography.heading_line_height(*level),
                ),
                context,
            ))
            .into_any_element(),
        Block::Quote(blocks) => div()
            .w_full()
            .border_l_2()
            .border_color(palette.accent)
            .pl_4()
            .py_1()
            .text_color(palette.muted_foreground)
            .child(render_blocks(blocks, text_index, context))
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
                            .text_color(palette.muted_foreground)
                            .child(if *ordered {
                                format!("{}.", start + index)
                            } else {
                                "•".into()
                            }),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(render_blocks(blocks, text_index, context)),
                    )
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
            let code_scroll = context
                .horizontal_scrolls
                .handle(format!("markdown-code:{message_id}:{content_text_index}"));
            let boundary_scroll = code_scroll.clone();

            div()
                .w_full()
                .rounded_lg()
                .border_1()
                .border_color(palette.border)
                .bg(palette.surface)
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
                        .border_color(palette.border)
                        .text_size(px(typography.micro_size))
                        .line_height(px(typography.micro_line_height))
                        .text_color(palette.muted_foreground)
                        .child(div().min_w_0().flex_1().child(selectable(
                            message_id,
                            language_text_index,
                            if language.is_empty() {
                                "Code".into()
                            } else {
                                language.clone()
                            },
                            selection,
                            palette,
                        )))
                        .child(CopyButton::new(copy_button_id, content_to_copy)),
                )
                .child(
                    div()
                        .id(element_key("code", content))
                        .track_scroll(&code_scroll)
                        .on_scroll_wheel(move |event, _, cx| {
                            if nested_horizontal_scroll_captures(event, &boundary_scroll) {
                                cx.stop_propagation();
                            }
                        })
                        .w_full()
                        .min_w_0()
                        .when(context.code_block_wrap, |element| {
                            element.overflow_hidden().whitespace_normal()
                        })
                        .when(!context.code_block_wrap, |element| {
                            element.overflow_x_scroll().whitespace_nowrap()
                        })
                        .p_3()
                        .font(theme::code_font(cx))
                        .text_size(px(typography.code_size))
                        .line_height(px(typography.code_line_height))
                        .child(
                            selectable(
                                message_id,
                                content_text_index,
                                content.clone(),
                                selection,
                                palette,
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
            text_index,
            context,
        ),
        Block::Rule => div()
            .w_full()
            .h(px(1.0))
            .my_2()
            .bg(palette.border)
            .into_any_element(),
    }
}

fn render_table(
    table: TableContent<'_>,
    text_index: &mut usize,
    context: &MarkdownContext<'_>,
) -> AnyElement {
    let typography = context.typography;
    let palette = context.palette;
    let table_scroll = context.horizontal_scrolls.handle(format!(
        "markdown-table:{}:{}",
        context.message_id, *text_index
    ));
    let boundary_scroll = table_scroll.clone();

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
                    .border_color(palette.border)
                    .p_2()
                    .when(header_row, |element| {
                        element
                            .bg(palette.surface)
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
                            text_index,
                            InlineMetrics::new(
                                typography.body_size,
                                typography.table_line_height(),
                            ),
                            context,
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
        .track_scroll(&table_scroll)
        .on_scroll_wheel(move |event, _, cx| {
            if nested_horizontal_scroll_captures(event, &boundary_scroll) {
                cx.stop_propagation();
            }
        })
        .w_full()
        .overflow_x_scroll()
        .restrict_scroll_to_axis()
        .rounded_lg()
        .border_1()
        .border_color(palette.border)
        .child(
            div()
                .flex()
                .flex_col()
                .children((!table.header.is_empty()).then(|| render_row(table.header, true)))
                .children(table.rows.iter().map(|row| render_row(row, false))),
        )
        .into_any_element()
}
