use gpui::{
    AnyElement, App, ClipboardItem, FontWeight, HighlightStyle, SharedString, div, prelude::*, px,
    rgba,
};
use gpui_component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
};

use super::{
    InlineMetrics, element_key, next_text_index, render_blocks, render_formula_element,
    render_inlines, selectable,
};
use crate::{
    desktop::ui::{
        icons::{AppIcon, IconTone, render_icon},
        selectable_text::TextSelection,
        theme,
        typography::MessageTypography,
    },
    markdown::{Block, Inline, TableAlignment},
};

struct TableContent<'a> {
    alignments: &'a [TableAlignment],
    header: &'a [Vec<Inline>],
    rows: &'a [Vec<Vec<Inline>>],
}

pub(super) fn render_block(
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
                        .font(theme::code_font(cx))
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
