use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use gpui::{AnyElement, App, SharedString, div, prelude::*, px};
use gpui_component::ActiveTheme as _;

use super::selectable_text::{SelectableText, TextSelection, selection_color};
use super::typography::MessageTypography;
use crate::markdown::{Block, MarkdownDocument};

mod block;
mod formula;
mod inline;

use block::render_block;
use formula::render_formula_element;
use inline::render_inlines;

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
