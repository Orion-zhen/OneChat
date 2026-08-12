use comrak::{Arena, format_html, nodes::NodeValue, parse_document};

use super::{formula::render_formula_cached, parser::options};

pub(crate) fn to_html(source: &str) -> String {
    let arena = Arena::new();
    let mut options = options();
    options.render.escape = true;
    let root = parse_document(&arena, source, &options);

    for node in root.descendants() {
        let formula = match &node.data.borrow().value {
            NodeValue::Math(math) => Some((math.literal.clone(), math.display_math)),
            NodeValue::CodeBlock(code)
                if matches!(
                    code.info
                        .split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .to_ascii_lowercase()
                        .as_str(),
                    "math" | "latex" | "tex"
                ) =>
            {
                Some((code.literal.clone(), true))
            }
            _ => None,
        };
        if let Some((source, display)) = formula {
            node.data.borrow_mut().value = NodeValue::Raw(formula_html(&source, display));
        }
    }

    let mut output = String::new();
    format_html(root, &options, &mut output).expect("writing HTML to memory cannot fail");
    output.replace(
        "<a href=\"",
        "<a target=\"_blank\" rel=\"noreferrer noopener\" href=\"",
    )
}

fn formula_html(source: &str, display: bool) -> String {
    match render_formula_cached(source.trim(), display, false, 2.0, 1.0) {
        Ok(image) => {
            let class = if display {
                "formula formula-display"
            } else {
                "formula formula-inline"
            };
            format!(
                "<span class=\"{class}\" style=\"--formula-width:{:.2}px;--formula-height:{:.2}px\">{}</span>",
                image.width,
                image.height,
                String::from_utf8_lossy(&image.svg),
            )
        }
        Err(_) => format!(
            "<code class=\"formula-source\">{}</code>",
            escape_html(source.trim())
        ),
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::to_html;

    #[test]
    fn html_escapes_raw_markup_and_renders_formula() {
        let html = to_html("<script>alert(1)</script>\n\n$x^2$\n\n[Link](https://example.com)");

        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("class=\"formula formula-inline\""));
        assert!(html.contains("target=\"_blank\""));
    }
}
