use comrak::{
    Arena, Options,
    nodes::{AstNode, ListType, NodeValue, TableAlignment as ComrakTableAlignment},
    parse_document,
};

use super::{Block, Formula, Inline, InlineStyle, MarkdownDocument, TableAlignment};

fn table_alignment(alignment: ComrakTableAlignment) -> TableAlignment {
    match alignment {
        ComrakTableAlignment::None => TableAlignment::None,
        ComrakTableAlignment::Left => TableAlignment::Left,
        ComrakTableAlignment::Center => TableAlignment::Center,
        ComrakTableAlignment::Right => TableAlignment::Right,
    }
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
        NodeValue::Table(table) => Some(parse_table(
            node,
            table.alignments.into_iter().map(table_alignment).collect(),
        )),
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

pub(crate) fn formula(source: &str, display: bool) -> Formula {
    Formula {
        source: source.trim().to_string(),
        display,
    }
}
