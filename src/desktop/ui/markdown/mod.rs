use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use gpui::{AnyElement, App, Hsla, Rgba, SharedString, div, prelude::*, px};
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
struct MarkdownPalette {
    foreground: Hsla,
    muted_foreground: Hsla,
    accent: Hsla,
    border: Hsla,
    surface: Hsla,
    selection: Rgba,
}

impl MarkdownPalette {
    fn assistant(cx: &App) -> Self {
        Self {
            foreground: cx.theme().foreground,
            muted_foreground: cx.theme().muted_foreground,
            accent: cx.theme().primary,
            border: cx.theme().border,
            surface: cx.theme().muted,
            selection: selection_color(cx.theme().is_dark()),
        }
    }

    fn user(cx: &App) -> Self {
        let palette = crate::desktop::ui::theme::user_message_palette(cx);
        Self {
            foreground: palette.foreground.into(),
            muted_foreground: palette.muted_foreground.into(),
            accent: palette.accent.into(),
            border: palette.border.into(),
            surface: palette.surface.into(),
            selection: palette.selection,
        }
    }
}

struct MarkdownOptions {
    palette: MarkdownPalette,
    code_block_wrap: bool,
}

struct MarkdownContext<'a> {
    message_id: &'a str,
    selection: &'a TextSelection,
    scale_factor: f32,
    typography: MessageTypography,
    palette: MarkdownPalette,
    code_block_wrap: bool,
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
    code_block_wrap: bool,
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
            code_block_wrap,
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
    code_block_wrap: bool,
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
            code_block_wrap,
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
    options: MarkdownOptions,
    cx: &App,
) -> AnyElement {
    let context = MarkdownContext {
        message_id,
        selection,
        scale_factor,
        typography,
        palette: options.palette,
        code_block_wrap: options.code_block_wrap,
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
    div()
        .whitespace_normal()
        .text_size(px(typography.body_size))
        .line_height(px(typography.body_line_height))
        .child(selectable(
            message_id,
            0,
            source.to_string(),
            selection,
            palette,
        ))
        .text_color(palette.foreground)
        .into_any_element()
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
    message_id: &str,
    index: usize,
    content: String,
    selection: &TextSelection,
    palette: MarkdownPalette,
) -> SelectableText {
    SelectableText::new(
        SharedString::from(format!("message-text-{message_id}-{index}")),
        content,
        selection.clone(),
        palette.selection,
    )
}

fn element_key(prefix: &str, content: &str) -> SharedString {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("markdown-{prefix}-{:x}", hasher.finish()).into()
}
