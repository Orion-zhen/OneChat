use std::sync::OnceLock;

use two_face::{
    re_exports::syntect::{
        easy::HighlightLines,
        highlighting::{Color, Theme},
        parsing::{SyntaxReference, SyntaxSet},
        util::LinesWithEndings,
    },
    theme::{EmbeddedLazyThemeSet, EmbeddedThemeName},
};

use super::{CodeHighlight, CodeHighlights};

struct Assets {
    syntaxes: SyntaxSet,
    themes: EmbeddedLazyThemeSet,
}

pub(super) fn highlight_code(language: &str, source: &str) -> CodeHighlights {
    if source.is_empty() {
        return CodeHighlights::default();
    }

    let assets = assets();
    let syntax = if language.is_empty() {
        assets.syntaxes.find_syntax_by_first_line(source)
    } else {
        assets.syntaxes.find_syntax_by_token(language)
    };
    let Some(syntax) = syntax else {
        return CodeHighlights::default();
    };

    CodeHighlights {
        light: highlight(
            source,
            syntax,
            assets.themes.get(EmbeddedThemeName::OneHalfLight),
            &assets.syntaxes,
        ),
        dark: highlight(
            source,
            syntax,
            assets.themes.get(EmbeddedThemeName::OneHalfDark),
            &assets.syntaxes,
        ),
    }
}

fn assets() -> &'static Assets {
    static ASSETS: OnceLock<Assets> = OnceLock::new();
    ASSETS.get_or_init(|| Assets {
        syntaxes: two_face::syntax::extra_newlines(),
        themes: two_face::theme::extra(),
    })
}

fn highlight(
    source: &str,
    syntax: &SyntaxReference,
    theme: &Theme,
    syntaxes: &SyntaxSet,
) -> Vec<CodeHighlight> {
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut highlights: Vec<CodeHighlight> = Vec::new();
    let mut offset = 0;

    for line in LinesWithEndings::from(source) {
        let Ok(regions) = highlighter.highlight_line(line, syntaxes) else {
            return Vec::new();
        };
        for (style, text) in regions {
            let start = offset;
            offset += text.len();
            let rgba = rgba(style.foreground);
            if let Some(previous) = highlights.last_mut()
                && previous.rgba == rgba
                && previous.range.end == start
            {
                previous.range.end = offset;
            } else if start != offset {
                highlights.push(CodeHighlight {
                    range: start..offset,
                    rgba,
                });
            }
        }
    }

    if offset == source.len() {
        highlights
    } else {
        Vec::new()
    }
}

fn rgba(color: Color) -> u32 {
    u32::from_be_bytes([color.r, color.g, color.b, color.a])
}
