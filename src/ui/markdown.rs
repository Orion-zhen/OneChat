use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Mutex, OnceLock},
};

use comrak::{
    Arena, Options,
    nodes::{AstNode, ListType, NodeValue, TableAlignment},
    parse_document,
};
use gpui::{AnyElement, FontWeight, Image, ImageFormat, SharedString, div, img, prelude::*, px};
use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_parser::parse;
use ratex_svg::{SvgColorSyntax, SvgOptions, render_to_svg_with_color_syntax};
use ratex_types::{color::Color, math_style::MathStyle};

use crate::ui::shell::Colors;

const MIN_FORMULA_SCALE: f32 = 2.0;

#[derive(Clone, Debug)]
pub struct MarkdownDocument {
    pub(crate) blocks: Vec<Block>,
}

#[derive(Clone, Debug)]
pub(crate) enum Block {
    Paragraph(Vec<Inline>),
    Heading(u8, Vec<Inline>),
    Quote(Vec<Block>),
    List {
        ordered: bool,
        start: usize,
        items: Vec<Vec<Block>>,
    },
    Code {
        language: String,
        content: String,
    },
    Formula(Formula),
    Table {
        alignments: Vec<TableAlignment>,
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Rule,
}

#[derive(Clone, Debug)]
pub(crate) enum Inline {
    Text { content: String, style: InlineStyle },
    Formula(Formula),
    Break,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct InlineStyle {
    emphasis: bool,
    strong: bool,
    strike: bool,
    code: bool,
    link: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Formula {
    pub source: String,
    pub display: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct FormulaImage {
    svg: Vec<u8>,
    width: f32,
    height: f32,
}

impl MarkdownDocument {
    pub fn parse(source: &str) -> Self {
        let arena = Arena::new();
        let mut options = Options::default();
        options.extension.strikethrough = true;
        options.extension.table = true;
        options.extension.autolink = true;
        options.extension.tasklist = true;
        options.extension.footnotes = true;
        options.extension.inline_footnotes = true;
        options.extension.description_lists = true;
        options.extension.alerts = true;
        options.extension.math_dollars = true;
        options.extension.math_latex = true;
        options.extension.math_code = true;
        options.extension.cjk_friendly_emphasis = true;

        let root = parse_document(&arena, source, &options);
        Self {
            blocks: parse_blocks(root),
        }
    }
}

fn parse_blocks<'a>(node: &'a AstNode<'a>) -> Vec<Block> {
    node.children().filter_map(parse_block).collect()
}

fn parse_block<'a>(node: &'a AstNode<'a>) -> Option<Block> {
    let value = node.data.borrow().value.clone();
    match value {
        NodeValue::Paragraph => {
            let inlines = parse_inlines(node);
            if inlines.len() == 1
                && let Inline::Formula(formula) = &inlines[0]
                && formula.display
            {
                return Some(Block::Formula(formula.clone()));
            }
            Some(Block::Paragraph(inlines))
        }
        NodeValue::Heading(heading) => Some(Block::Heading(heading.level, parse_inlines(node))),
        NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) | NodeValue::Alert(_) => {
            Some(Block::Quote(parse_blocks(node)))
        }
        NodeValue::List(list) => Some(Block::List {
            ordered: list.list_type == ListType::Ordered,
            start: list.start.max(1),
            items: node.children().map(parse_blocks).collect(),
        }),
        NodeValue::CodeBlock(code) => {
            let language = code
                .info
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_lowercase();
            if matches!(language.as_str(), "math" | "latex" | "tex") {
                Some(Block::Formula(formula(&code.literal, true)))
            } else {
                Some(Block::Code {
                    language,
                    content: code.literal,
                })
            }
        }
        NodeValue::Table(table) => Some(parse_table(node, table.alignments)),
        NodeValue::ThematicBreak => Some(Block::Rule),
        NodeValue::HtmlBlock(html) => Some(Block::Code {
            language: "html".into(),
            content: html.literal,
        }),
        NodeValue::DescriptionList
        | NodeValue::DescriptionItem(_)
        | NodeValue::DescriptionTerm
        | NodeValue::DescriptionDetails
        | NodeValue::FootnoteDefinition(_)
        | NodeValue::Item(_)
        | NodeValue::Document => Some(Block::Quote(parse_blocks(node))),
        NodeValue::TaskItem(task) => {
            let mut inlines = vec![Inline::Text {
                content: if task.symbol.is_some() {
                    "☑ "
                } else {
                    "☐ "
                }
                .into(),
                style: InlineStyle::default(),
            }];
            inlines.extend(parse_inlines(node));
            Some(Block::Paragraph(inlines))
        }
        NodeValue::Math(math) => Some(Block::Formula(formula(&math.literal, math.display_math))),
        _ => {
            let inlines = parse_inlines(node);
            (!inlines.is_empty()).then_some(Block::Paragraph(inlines))
        }
    }
}

fn parse_table<'a>(node: &'a AstNode<'a>, alignments: Vec<TableAlignment>) -> Block {
    let mut header = Vec::new();
    let mut rows = Vec::new();
    for row in node.children() {
        let is_header = matches!(row.data.borrow().value, NodeValue::TableRow(true));
        let cells = row.children().map(parse_inlines).collect::<Vec<_>>();
        if is_header {
            header = cells;
        } else {
            rows.push(cells);
        }
    }
    Block::Table {
        alignments,
        header,
        rows,
    }
}

fn parse_inlines<'a>(node: &'a AstNode<'a>) -> Vec<Inline> {
    let mut inlines = Vec::new();
    collect_inlines(node, InlineStyle::default(), &mut inlines);
    inlines
}

fn collect_inlines<'a>(node: &'a AstNode<'a>, style: InlineStyle, output: &mut Vec<Inline>) {
    for child in node.children() {
        let value = child.data.borrow().value.clone();
        match value {
            NodeValue::Text(text) => push_text(output, text.into_owned(), style),
            NodeValue::Code(code) => push_text(
                output,
                code.literal,
                InlineStyle {
                    code: true,
                    ..style
                },
            ),
            NodeValue::Emph => collect_inlines(
                child,
                InlineStyle {
                    emphasis: true,
                    ..style
                },
                output,
            ),
            NodeValue::Strong => collect_inlines(
                child,
                InlineStyle {
                    strong: true,
                    ..style
                },
                output,
            ),
            NodeValue::Strikethrough => collect_inlines(
                child,
                InlineStyle {
                    strike: true,
                    ..style
                },
                output,
            ),
            NodeValue::Link(_) | NodeValue::WikiLink(_) => collect_inlines(
                child,
                InlineStyle {
                    link: true,
                    ..style
                },
                output,
            ),
            NodeValue::Image(_) => {
                push_text(output, "Image: ".into(), style);
                collect_inlines(
                    child,
                    InlineStyle {
                        link: true,
                        ..style
                    },
                    output,
                );
            }
            NodeValue::Math(math) => {
                output.push(Inline::Formula(formula(&math.literal, math.display_math)))
            }
            NodeValue::SoftBreak => push_text(output, " ".into(), style),
            NodeValue::LineBreak => output.push(Inline::Break),
            NodeValue::HtmlInline(html) => push_text(
                output,
                html,
                InlineStyle {
                    code: true,
                    ..style
                },
            ),
            NodeValue::FootnoteReference(reference) => push_text(
                output,
                format!("[{}]", reference.ref_num),
                InlineStyle {
                    link: true,
                    ..style
                },
            ),
            _ => collect_inlines(child, style, output),
        }
    }
}

fn push_text(output: &mut Vec<Inline>, content: String, style: InlineStyle) {
    if content.is_empty() {
        return;
    }
    if let Some(Inline::Text {
        content: previous,
        style: previous_style,
    }) = output.last_mut()
        && same_style(*previous_style, style)
    {
        previous.push_str(&content);
    } else {
        output.push(Inline::Text { content, style });
    }
}

fn same_style(left: InlineStyle, right: InlineStyle) -> bool {
    left.emphasis == right.emphasis
        && left.strong == right.strong
        && left.strike == right.strike
        && left.code == right.code
        && left.link == right.link
}

fn formula(source: &str, display: bool) -> Formula {
    Formula {
        source: source.trim().to_string(),
        display,
    }
}

fn render_formula_cached(
    source: &str,
    display: bool,
    dark: bool,
    scale_factor: f32,
) -> Result<FormulaImage, String> {
    type FormulaKey = (String, bool, bool, u32);
    static CACHE: OnceLock<Mutex<HashMap<FormulaKey, Result<FormulaImage, String>>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let raster_scale = if scale_factor.is_finite() {
        scale_factor.max(MIN_FORMULA_SCALE)
    } else {
        MIN_FORMULA_SCALE
    };
    let key = (source.to_string(), display, dark, raster_scale.to_bits());
    if let Some(cached) = cache
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&key)
    {
        return cached.clone();
    }

    let rendered = catch_unwind(AssertUnwindSafe(|| {
        render_formula(source, display, dark, raster_scale)
    }))
    .map_err(|_| "Formula renderer stopped unexpectedly".to_string())
    .and_then(|result| result);
    cache
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(key, rendered.clone());
    rendered
}

fn render_formula(
    source: &str,
    display: bool,
    dark: bool,
    raster_scale: f32,
) -> Result<FormulaImage, String> {
    if source.is_empty() {
        return Err("Formula is empty".into());
    }
    let ast = parse(source).map_err(|error| error.to_string())?;
    let layout_options = LayoutOptions {
        style: if display {
            MathStyle::Display
        } else {
            MathStyle::Text
        },
        color: if dark { Color::WHITE } else { Color::BLACK },
        ..LayoutOptions::default()
    };
    let layout = layout(&ast, &layout_options);
    let display_list = to_display_list(&layout);
    let font_size = if display { 22.0 } else { 17.0 };
    let padding = if display { 6.0 } else { 2.0 };
    let raster_scale = f64::from(raster_scale);
    let options = SvgOptions {
        font_size: font_size * raster_scale,
        padding: padding * raster_scale,
        stroke_width: raster_scale,
        embed_glyphs: true,
        font_dir: String::new(),
    };
    let svg = render_to_svg_with_color_syntax(&display_list, &options, SvgColorSyntax::Rgb)
        .replace("pt\"", "px\"");
    let width = (display_list.width * font_size + padding * 2.0).max(font_size) as f32;
    let height = ((display_list.height + display_list.depth) * font_size + padding * 2.0)
        .max(font_size) as f32;
    Ok(FormulaImage {
        svg: svg.into_bytes(),
        width,
        height,
    })
}

pub(crate) fn render(document: &MarkdownDocument, colors: Colors, scale_factor: f32) -> AnyElement {
    render_blocks(&document.blocks, colors, scale_factor)
}

pub(crate) fn render_plain(source: &str, colors: Colors) -> AnyElement {
    div()
        .whitespace_normal()
        .line_height(px(24.0))
        .child(source.to_string())
        .text_color(colors.text)
        .into_any_element()
}

fn render_blocks(blocks: &[Block], colors: Colors, scale_factor: f32) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        .children(
            blocks
                .iter()
                .map(|block| render_block(block, colors, scale_factor)),
        )
        .into_any_element()
}

fn render_block(block: &Block, colors: Colors, scale_factor: f32) -> AnyElement {
    match block {
        Block::Paragraph(inlines) => render_inlines(inlines, colors, scale_factor, false),
        Block::Heading(level, inlines) => div()
            .pt(if *level <= 2 { px(8.0) } else { px(4.0) })
            .text_size(match level {
                1 => px(24.0),
                2 => px(21.0),
                3 => px(18.0),
                _ => px(16.0),
            })
            .font_weight(FontWeight::SEMIBOLD)
            .child(render_inlines(inlines, colors, scale_factor, false))
            .into_any_element(),
        Block::Quote(blocks) => div()
            .w_full()
            .border_l_2()
            .border_color(colors.accent)
            .pl_4()
            .py_1()
            .text_color(colors.muted)
            .child(render_blocks(blocks, colors, scale_factor))
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
                            .text_color(colors.muted)
                            .child(if *ordered {
                                format!("{}.", start + index)
                            } else {
                                "•".into()
                            }),
                    )
                    .child(div().min_w_0().flex_1().child(render_blocks(
                        blocks,
                        colors,
                        scale_factor,
                    )))
            }))
            .into_any_element(),
        Block::Code { language, content } => div()
            .w_full()
            .rounded_lg()
            .border_1()
            .border_color(colors.border)
            .bg(colors.raised)
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(colors.border)
                    .text_xs()
                    .text_color(colors.muted)
                    .child(if language.is_empty() {
                        "Code".into()
                    } else {
                        language.clone()
                    }),
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
                    .child(content.clone()),
            )
            .into_any_element(),
        Block::Formula(formula) => render_formula_element(formula, colors, scale_factor),
        Block::Table {
            alignments,
            header,
            rows,
        } => render_table(alignments, header, rows, colors, scale_factor),
        Block::Rule => div()
            .w_full()
            .h(px(1.0))
            .my_2()
            .bg(colors.border)
            .into_any_element(),
    }
}

fn render_inlines(
    inlines: &[Inline],
    colors: Colors,
    scale_factor: f32,
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
                            .bg(colors.raised)
                            .px_1()
                            .font_family("SFMono-Regular")
                            .text_sm()
                    })
                    .when(style.link, |element| element.text_color(colors.accent))
                    .child(content)
                    .into_any_element()
            }
            Inline::Formula(formula) => render_formula_element(formula, colors, scale_factor),
            Inline::Break => div().w_full().h(px(1.0)).into_any_element(),
        }))
        .into_any_element()
}

fn render_formula_element(formula: &Formula, colors: Colors, scale_factor: f32) -> AnyElement {
    match render_formula_cached(&formula.source, formula.display, colors.dark, scale_factor) {
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
            .border_color(colors.danger)
            .bg(colors.raised)
            .px_2()
            .py_1()
            .text_sm()
            .text_color(colors.danger)
            .child(format!("{} · {error}", formula.source))
            .into_any_element(),
    }
}

fn render_table(
    alignments: &[TableAlignment],
    header: &[Vec<Inline>],
    rows: &[Vec<Vec<Inline>>],
    colors: Colors,
    scale_factor: f32,
) -> AnyElement {
    let columns = header
        .len()
        .max(rows.iter().map(Vec::len).max().unwrap_or_default());
    let render_row = |cells: &[Vec<Inline>], header_row: bool| {
        div()
            .min_w(px(columns.max(1) as f32 * 130.0))
            .flex()
            .children((0..columns).map(|index| {
                let alignment = alignments
                    .get(index)
                    .copied()
                    .unwrap_or(TableAlignment::None);
                div()
                    .min_w(px(130.0))
                    .flex_1()
                    .border_r_1()
                    .border_b_1()
                    .border_color(colors.border)
                    .p_2()
                    .when(header_row, |element| {
                        element.bg(colors.raised).font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(alignment == TableAlignment::Center, |element| {
                        element.text_center()
                    })
                    .when(alignment == TableAlignment::Right, |element| {
                        element.text_right()
                    })
                    .children(
                        cells
                            .get(index)
                            .map(|cell| render_inlines(cell, colors, scale_factor, true)),
                    )
            }))
    };

    div()
        .id(element_key(
            "table",
            header
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
        .border_color(colors.border)
        .child(
            div()
                .flex()
                .flex_col()
                .children((!header.is_empty()).then(|| render_row(header, true)))
                .children(rows.iter().map(|row| render_row(row, false))),
        )
        .into_any_element()
}

fn element_key(prefix: &str, content: &str) -> SharedString {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("markdown-{prefix}-{:x}", hasher.finish()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_document_blocks_and_math() {
        let markdown = r#"
# Heading

A **bold** paragraph with $x^2 + y^2$.

> quote

- one
- two

| A | B |
|---|---:|
| 1 | 2 |

```rust
fn main() {}
```

$$\frac{1}{2}$$
"#;
        let document = MarkdownDocument::parse(markdown);

        assert!(
            document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::Heading(1, _)))
        );
        assert!(
            document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::Quote(_)))
        );
        assert!(
            document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::List { .. }))
        );
        assert!(
            document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::Table { .. }))
        );
        assert!(
            document
                .blocks
                .iter()
                .any(|block| matches!(block, Block::Code { language, .. } if language == "rust"))
        );
        let formula = document
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::Formula(formula) => Some(formula),
                _ => None,
            })
            .unwrap();
        let rendered = render_formula_cached(&formula.source, formula.display, false, 1.0).unwrap();
        let svg = std::str::from_utf8(&rendered.svg).unwrap();
        assert!(svg.contains("<path"));
        assert!(!svg.contains("<text"));
        assert!(document.blocks.iter().any(|block| matches!(block, Block::Paragraph(inlines) if inlines.iter().any(|inline| matches!(inline, Inline::Formula(_))))));
    }

    #[test]
    fn supports_latex_delimiters_and_formula_errors() {
        let document = MarkdownDocument::parse(r"Inline \(a+b\) and display \[c+d\].");
        let formulas = document
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(inlines) => Some(inlines),
                _ => None,
            })
            .flatten()
            .filter_map(|inline| match inline {
                Inline::Formula(formula) => Some(formula),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(formulas.len(), 2);
        assert!(formulas.iter().all(|formula| {
            render_formula_cached(&formula.source, formula.display, false, 2.0).is_ok()
        }));

        let invalid = formula(r"\not-a-real-command{", true);
        assert!(render_formula_cached(&invalid.source, invalid.display, false, 2.0).is_err());
    }

    #[test]
    fn formula_svg_adapts_to_theme_and_display_scale() {
        fn view_box_size(image: &FormulaImage) -> (f32, f32) {
            let svg = std::str::from_utf8(&image.svg).unwrap();
            let size = svg
                .split_once("viewBox=\"0 0 ")
                .unwrap()
                .1
                .split_once('"')
                .unwrap()
                .0
                .split_whitespace()
                .map(|value| value.parse::<f32>().unwrap())
                .collect::<Vec<_>>();
            (size[0], size[1])
        }

        let light = render_formula_cached("x^2", false, false, 1.0).unwrap();
        let dark = render_formula_cached("x^2", false, true, 3.0).unwrap();
        let light_svg = std::str::from_utf8(&light.svg).unwrap();
        let dark_svg = std::str::from_utf8(&dark.svg).unwrap();
        let light_view_box = view_box_size(&light);
        let dark_view_box = view_box_size(&dark);

        assert!(light_svg.contains("rgb(0,0,0)"));
        assert!(dark_svg.contains("rgb(255,255,255)"));
        assert_eq!((light.width, light.height), (dark.width, dark.height));
        assert!((light_view_box.0 / light.width - 2.0).abs() < 0.01);
        assert!((light_view_box.1 / light.height - 2.0).abs() < 0.01);
        assert!((dark_view_box.0 / dark.width - 3.0).abs() < 0.01);
        assert!((dark_view_box.1 / dark.height - 3.0).abs() < 0.01);
    }

    #[test]
    fn long_streamed_markdown_shape_parses_without_losing_blocks() {
        let mut markdown = String::new();
        for index in 0..120 {
            markdown.push_str(&format!(
                "## Section {index}\n\n- item one\n- item two\n\n| x | y |\n|---|---|\n| {index} | $x_{index}^2$ |\n\n"
            ));
        }
        let document = MarkdownDocument::parse(&markdown);
        assert_eq!(
            document
                .blocks
                .iter()
                .filter(|block| matches!(block, Block::Heading(2, _)))
                .count(),
            120
        );
        assert_eq!(
            document
                .blocks
                .iter()
                .filter(|block| matches!(block, Block::Table { .. }))
                .count(),
            120
        );
    }

    #[test]
    fn raw_html_is_kept_as_text_instead_of_rendered() {
        let document = MarkdownDocument::parse("<script>alert('no')</script>");
        assert!(matches!(
            document.blocks.as_slice(),
            [Block::Code { language, content }] if language == "html" && content.contains("script")
        ));
    }
}
