use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use gpui::{AnyElement, App, Hsla, Rgba, SharedString, div, prelude::*, px};

use super::selectable_text::{SelectableText, SelectionGroup, TextSelection, selection_color};
use super::stream::HorizontalScrollRegistry;
use super::typography::MessageTypography;
use crate::markdown::{Block, MarkdownDocument};

mod block;
mod formula;
mod inline;

use block::render_block;
use formula::{formula_copy_text, render_formula_element};
use inline::render_inlines;

#[derive(Clone, Copy)]
struct MarkdownPalette {
    foreground: Hsla,
    muted_foreground: Hsla,
    accent: Hsla,
    soft_emphasis: Hsla,
    border: Hsla,
    surface: Hsla,
    selection: Rgba,
}

impl MarkdownPalette {
    fn assistant(cx: &App) -> Self {
        let palette = crate::desktop::ui::theme::palette(cx);
        Self {
            foreground: palette.foreground,
            muted_foreground: palette.muted_foreground,
            accent: palette.link,
            soft_emphasis: palette.emphasis,
            border: palette.border,
            surface: palette.raised,
            selection: selection_color(cx),
        }
    }

    fn user(cx: &App) -> Self {
        let palette = crate::desktop::ui::theme::palette(cx).user_message;
        Self {
            foreground: palette.foreground,
            muted_foreground: palette.muted_foreground,
            accent: palette.link,
            soft_emphasis: palette.emphasis,
            border: palette.border,
            surface: palette.surface,
            selection: palette.selection.into(),
        }
    }
}

struct MarkdownOptions<'a> {
    palette: MarkdownPalette,
    behavior: MarkdownBehavior<'a>,
}

pub(crate) struct MarkdownBehavior<'a> {
    pub(crate) code_block_wrap: bool,
    pub(crate) horizontal_scrolls: &'a HorizontalScrollRegistry,
}

struct MarkdownContext<'a> {
    message_id: &'a str,
    selection: &'a TextSelection,
    scale_factor: f32,
    typography: MessageTypography,
    palette: MarkdownPalette,
    code_block_wrap: bool,
    horizontal_scrolls: &'a HorizontalScrollRegistry,
    cx: &'a App,
}

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
    behavior: MarkdownBehavior<'_>,
    cx: &App,
) -> AnyElement {
    render_with_palette(
        document,
        message_id,
        selection,
        scale_factor,
        typography,
        MarkdownOptions {
            palette: MarkdownPalette::assistant(cx),
            behavior,
        },
        cx,
    )
}

pub(crate) fn render_user(
    document: &MarkdownDocument,
    message_id: &str,
    selection: &TextSelection,
    scale_factor: f32,
    typography: MessageTypography,
    behavior: MarkdownBehavior<'_>,
    cx: &App,
) -> AnyElement {
    render_with_palette(
        document,
        message_id,
        selection,
        scale_factor,
        typography,
        MarkdownOptions {
            palette: MarkdownPalette::user(cx),
            behavior,
        },
        cx,
    )
}

fn render_with_palette(
    document: &MarkdownDocument,
    message_id: &str,
    selection: &TextSelection,
    scale_factor: f32,
    typography: MessageTypography,
    options: MarkdownOptions<'_>,
    cx: &App,
) -> AnyElement {
    let context = MarkdownContext {
        message_id,
        selection,
        scale_factor,
        typography,
        palette: options.palette,
        code_block_wrap: options.behavior.code_block_wrap,
        horizontal_scrolls: options.behavior.horizontal_scrolls,
        cx,
    };
    let mut text_index = 0;
    render_blocks(&document.blocks, &mut text_index, &context)
}

pub(crate) fn render_plain(
    source: &str,
    message_id: &str,
    selection: &TextSelection,
    typography: MessageTypography,
    cx: &App,
) -> AnyElement {
    render_plain_with_palette(
        source,
        message_id,
        selection,
        typography,
        MarkdownPalette::assistant(cx),
    )
}

pub(crate) fn render_user_plain(
    source: &str,
    message_id: &str,
    selection: &TextSelection,
    typography: MessageTypography,
    cx: &App,
) -> AnyElement {
    render_plain_with_palette(
        source,
        message_id,
        selection,
        typography,
        MarkdownPalette::user(cx),
    )
}

fn render_plain_with_palette(
    source: &str,
    message_id: &str,
    selection: &TextSelection,
    typography: MessageTypography,
    palette: MarkdownPalette,
) -> AnyElement {
    let group = selection.group(format!("plain-{message_id}"));
    let content = div()
        .whitespace_normal()
        .text_size(px(typography.body_size))
        .line_height(px(typography.body_line_height))
        .child(selectable(&group, 0, source.to_string(), palette))
        .text_color(palette.foreground);
    group.wrap(content)
}

fn render_blocks(
    blocks: &[Block],
    text_index: &mut usize,
    context: &MarkdownContext<'_>,
) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        .text_size(px(context.typography.body_size))
        .line_height(px(context.typography.body_line_height))
        .children(
            blocks
                .iter()
                .map(|block| render_block(block, text_index, context)),
        )
        .into_any_element()
}

fn next_text_index(index: &mut usize) -> usize {
    let current = *index;
    *index += 1;
    current
}

fn selectable(
    group: &SelectionGroup,
    index: usize,
    content: String,
    palette: MarkdownPalette,
) -> SelectableText {
    SelectableText::new(group.clone(), index as u64, content, palette.selection)
}

fn element_key(prefix: &str, content: &str) -> SharedString {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("markdown-{prefix}-{:x}", hasher.finish()).into()
}
