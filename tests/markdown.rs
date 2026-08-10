use onechat::markdown::{Block, Inline, MarkdownDocument, TableAlignment, render_formula_cached};

#[test]
fn markdown_parses_representative_chat_content() {
    let document = MarkdownDocument::parse(
        r#"# Title

Text with **strong** and $x^2$.

1. first
2. second

| Left | Right |
| :--- | ---: |
| A | B |

```rust
fn main() {}
```

$$E = mc^2$$
"#,
    );

    assert!(matches!(
        document.blocks.first(),
        Some(Block::Heading(1, inlines))
            if matches!(inlines.as_slice(), [Inline::Text { content, .. }] if content == "Title")
    ));
    assert!(document.blocks.iter().any(|block| matches!(
        block,
        Block::Paragraph(inlines)
            if inlines.iter().any(|inline| matches!(
                inline,
                Inline::Text { content, style } if content == "strong" && style.strong
            ))
            && inlines.iter().any(|inline| matches!(inline, Inline::Formula(formula) if formula.source == "x^2"))
    )));
    assert!(document.blocks.iter().any(|block| matches!(
        block,
        Block::List { ordered: true, start: 1, items } if items.len() == 2
    )));
    assert!(document.blocks.iter().any(|block| matches!(
        block,
        Block::Table { alignments, header, rows }
            if alignments == &[TableAlignment::Left, TableAlignment::Right]
                && header.len() == 2
                && rows.len() == 1
    )));
    assert!(document.blocks.iter().any(|block| matches!(
        block,
        Block::Code { language, content, .. }
            if language == "rust" && content == "fn main() {}"
    )));
    assert!(document.blocks.iter().any(|block| matches!(
        block,
        Block::Formula(formula) if formula.display && formula.source == "E = mc^2"
    )));
}

#[test]
fn formula_renderer_returns_a_displayable_svg() {
    let formula = render_formula_cached("x^2 + y^2", true, false, 2.0, 1.0).unwrap();
    assert!(formula.width > 0.0);
    assert!(formula.height > 0.0);
    assert!(String::from_utf8_lossy(&formula.svg).contains("<svg"));
}
