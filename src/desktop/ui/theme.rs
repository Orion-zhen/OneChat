use std::rc::Rc;

use gpui::{
    App, Font, FontFallbacks, Global, Hsla, Rgba, SharedString, Window, WindowAppearance, font,
    rgba,
};
use gpui_component::{
    Colorize as _, Theme as ComponentTheme, ThemeConfig, ThemeConfigColors,
    ThemeMode as ComponentThemeMode, scroll::ScrollbarMode,
};

use crate::domain::{
    DEFAULT_CODE_FONT_FAMILY, DEFAULT_THEME_COLOR, DEFAULT_UI_FONT_FAMILY, Theme,
    normalize_font_families,
};
mod component;
mod fonts;
mod palette;

pub(crate) use component::{component_mode, init, sync_component_theme, window_background};
pub(crate) use fonts::{code_font, sync_fonts, ui_font};
pub(crate) use palette::{AppPalette, palette, parse_theme_color};

use fonts::app_fonts;
use palette::ActiveAppPalette;
#[cfg(test)]
use palette::contrast_ratio;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_text_colors_remain_readable() {
        for seed in ["#007AFF", "#FFD60A", "#34C759", "#AF52DE", "#808080"] {
            for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
                let palette = AppPalette::generate(mode, parse_theme_color(seed));
                assert!(contrast_ratio(palette.link, palette.canvas) >= 4.5);
                assert!(
                    contrast_ratio(palette.user_message.link, palette.user_message.background)
                        >= 4.5
                );
                assert!(contrast_ratio(palette.on_accent, palette.accent) >= 4.5);
            }
        }
    }

    #[test]
    fn palette_has_distinct_interaction_states_and_control_thumb() {
        for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
            let palette = AppPalette::generate(mode, parse_theme_color(DEFAULT_THEME_COLOR));
            assert_ne!(palette.accent, palette.accent_hover);
            assert_ne!(palette.accent, palette.accent_active);
            assert_ne!(palette.secondary, palette.secondary_hover);
            assert_ne!(palette.secondary_hover, palette.secondary_active);
            assert_eq!(palette.control_thumb, Hsla::white());
        }
    }

    #[test]
    fn scrollbar_colors_are_subtle_readable_and_stateful() {
        for seed in ["#007AFF", "#FFD60A", "#34C759", "#AF52DE", "#808080"] {
            for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
                let palette = AppPalette::generate(mode, parse_theme_color(seed));
                for background in [
                    palette.canvas,
                    palette.panel,
                    palette.user_message.background,
                ] {
                    let normal = contrast_ratio(palette.scrollbar_thumb, background);
                    let hover = contrast_ratio(palette.scrollbar_thumb_hover, background);
                    let active = contrast_ratio(palette.scrollbar_thumb_active, background);

                    assert!(normal >= 3.0);
                    assert!(hover >= 4.5);
                    assert!(active >= 6.0);
                    assert!(normal < hover && hover < active);
                }
            }
        }
    }

    #[test]
    fn perceptual_adaptation_preserves_muted_theme_colors() {
        let seed = parse_theme_color("#A9B8C7");
        for mode in [ComponentThemeMode::Light, ComponentThemeMode::Dark] {
            let palette = AppPalette::generate(mode, seed);
            assert!(palette.accent.s < 0.55);
        }
    }

    #[test]
    fn invalid_theme_color_uses_default() {
        assert_eq!(
            parse_theme_color("not-a-color"),
            parse_theme_color(DEFAULT_THEME_COLOR)
        );
    }
}
