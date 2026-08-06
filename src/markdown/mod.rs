mod ast;
mod formula;
mod parser;

pub use ast::*;
pub use formula::{FormulaImage, render_formula_cached};
#[cfg(test)]
pub(crate) use parser::formula;

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
