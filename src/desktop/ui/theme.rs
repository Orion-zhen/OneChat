use std::rc::Rc;

use gpui::{App, Font, FontFallbacks, Global, Hsla, SharedString, Window, WindowAppearance, font};
use gpui_component::{
    Theme as ComponentTheme, ThemeConfig, ThemeConfigColors, ThemeMode as ComponentThemeMode,
    scroll::ScrollbarShow,
};

use crate::domain::{
    DEFAULT_CODE_FONT_FAMILY, DEFAULT_UI_FONT_FAMILY, Theme, normalize_font_families,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct AppFonts {
    ui: Font,
    code: Font,
}

impl Global for AppFonts {}

fn font_stack(families: &[String], default: &str) -> Font {
    let families = normalize_font_families(families.to_vec(), default);
    let mut result = font(families[0].clone());
    if families.len() > 1 {
        result.fallbacks = Some(FontFallbacks::from_fonts(families[1..].to_vec()));
    }
    result
}

fn app_fonts(ui_families: &[String], code_families: &[String]) -> AppFonts {
    AppFonts {
        ui: font_stack(ui_families, DEFAULT_UI_FONT_FAMILY),
        code: font_stack(code_families, DEFAULT_CODE_FONT_FAMILY),
    }
}

#[derive(Clone, Copy)]
struct Palette {
    canvas: &'static str,
    sidebar: &'static str,
    toolbar: &'static str,
    panel: &'static str,
    raised: &'static str,
    secondary: &'static str,
    hover: &'static str,
    text: &'static str,
    muted: &'static str,
    border: &'static str,
    accent: &'static str,
    accent_soft: &'static str,
    on_accent: &'static str,
    danger: &'static str,
    success: &'static str,
    scrim: &'static str,
}

const DARK: Palette = Palette {
    canvas: "#18181AF2",
    sidebar: "#242426DC",
    toolbar: "#1D1D1FDC",
    panel: "#2C2C2EEC",
    raised: "#FFFFFF10",
    secondary: "#FFFFFF10",
    hover: "#FFFFFF18",
    text: "#F5F5F7",
    muted: "#A1A1AA",
    border: "#FFFFFF16",
    accent: "#0A84FF",
    accent_soft: "#0A84FF2E",
    on_accent: "#FFFFFF",
    danger: "#FF453A",
    success: "#30D158",
    scrim: "#00000070",
};

const LIGHT: Palette = Palette {
    canvas: "#F5F5F7F2",
    sidebar: "#EBEBF0DC",
    toolbar: "#F7F7F8DC",
    panel: "#FFFFFFEC",
    raised: "#FFFFFF52",
    secondary: "#3C3C4324",
    hover: "#3C3C4324",
    text: "#1D1D1F",
    muted: "#48484A",
    border: "#3C3C4330",
    accent: "#007AFF",
    accent_soft: "#007AFF1F",
    on_accent: "#FFFFFF",
    danger: "#D70015",
    success: "#248A3D",
    scrim: "#00000052",
};

fn color(value: &'static str) -> Option<SharedString> {
    Some(value.into())
}

fn component_config(mode: ComponentThemeMode) -> Rc<ThemeConfig> {
    let palette = if mode.is_dark() { DARK } else { LIGHT };
    let mut colors = ThemeConfigColors::default();
    macro_rules! set_colors {
        ($($field:ident: $value:expr),+ $(,)?) => {
            $(colors.$field = color($value);)+
        };
    }
    set_colors! {
        background: palette.canvas,
        foreground: palette.text,
        border: palette.border,
        input: palette.border,
        muted: palette.raised,
        muted_foreground: palette.muted,
        accent: palette.accent_soft,
        accent_foreground: palette.text,
        primary: palette.accent,
        primary_active: palette.accent,
        primary_hover: palette.accent,
        primary_foreground: palette.on_accent,
        secondary: palette.secondary,
        secondary_active: palette.hover,
        secondary_hover: palette.hover,
        secondary_foreground: palette.text,
        danger: palette.danger,
        danger_foreground: palette.on_accent,
        success: palette.success,
        success_foreground: palette.on_accent,
        button: palette.raised,
        button_active: palette.accent_soft,
        button_foreground: palette.text,
        button_hover: palette.hover,
        button_primary: palette.accent,
        button_primary_active: palette.accent,
        button_primary_foreground: palette.on_accent,
        button_primary_hover: palette.accent,
        caret: palette.accent,
        ring: palette.accent,
        selection: palette.accent_soft,
        list: palette.panel,
        list_active: palette.accent_soft,
        list_active_border: palette.accent,
        list_even: palette.panel,
        list_head: palette.panel,
        list_hover: palette.hover,
        popover: palette.panel,
        popover_foreground: palette.text,
        sidebar: palette.sidebar,
        sidebar_accent: palette.accent_soft,
        sidebar_accent_foreground: palette.text,
        sidebar_border: palette.border,
        sidebar_foreground: palette.text,
        sidebar_primary: palette.accent,
        sidebar_primary_foreground: palette.on_accent,
        title_bar: palette.toolbar,
        title_bar_border: palette.border,
        overlay: palette.scrim,
        group_box: palette.panel,
        group_box_foreground: palette.text,
        slider_bar: palette.accent,
        slider_thumb: palette.on_accent,
        switch: palette.raised,
        switch_thumb: palette.on_accent,
        tab: palette.panel,
        tab_active: palette.panel,
        tab_active_foreground: palette.text,
        tab_bar: palette.toolbar,
        tab_bar_segmented: palette.raised,
        tab_foreground: palette.muted,
    }

    Rc::new(ThemeConfig {
        name: if mode.is_dark() {
            "OneChat Dark".into()
        } else {
            "OneChat Light".into()
        },
        mode,
        font_size: Some(14.0),
        font_family: Some(".SystemUIFont".into()),
        mono_font_family: Some(DEFAULT_CODE_FONT_FAMILY.into()),
        mono_font_size: Some(13.0),
        radius: Some(10),
        radius_lg: Some(16),
        shadow: Some(true),
        colors,
        ..Default::default()
    })
}

pub(crate) fn init(cx: &mut App) {
    let light = component_config(ComponentThemeMode::Light);
    let dark = component_config(ComponentThemeMode::Dark);
    let initial_mode = ComponentThemeMode::from(cx.window_appearance());
    let theme = ComponentTheme::global_mut(cx);
    theme.apply_config(&light);
    theme.apply_config(&dark);
    theme.apply_config(if initial_mode.is_dark() {
        &dark
    } else {
        &light
    });
    theme.list.active_highlight = false;
    theme.scrollbar_show = ScrollbarShow::Always;
    cx.set_global(app_fonts(&[], &[]));
}

pub(crate) fn sync_fonts(ui_families: &[String], code_families: &[String], cx: &mut App) {
    let next = app_fonts(ui_families, code_families);
    let fonts_changed = cx.global::<AppFonts>() != &next;
    let component_changed = ComponentTheme::global(cx).font_family != next.ui.family
        || ComponentTheme::global(cx).mono_font_family != next.code.family;
    if fonts_changed {
        *cx.global_mut::<AppFonts>() = next.clone();
    }
    if component_changed {
        let theme = ComponentTheme::global_mut(cx);
        theme.font_family = next.ui.family.clone();
        theme.mono_font_family = next.code.family.clone();
    }
    if fonts_changed || component_changed {
        cx.refresh_windows();
    }
}

pub(crate) fn ui_font(cx: &App) -> Font {
    cx.global::<AppFonts>().ui.clone()
}

pub(crate) fn code_font(cx: &App) -> Font {
    cx.global::<AppFonts>().code.clone()
}

pub(crate) fn component_mode(theme: Theme, appearance: WindowAppearance) -> ComponentThemeMode {
    match theme {
        Theme::Light => ComponentThemeMode::Light,
        Theme::Dark => ComponentThemeMode::Dark,
        Theme::System => appearance.into(),
    }
}

fn glass_alpha(mode: ComponentThemeMode, intensity: f32) -> f32 {
    let intensity = intensity.clamp(0.0, 1.0);
    if mode.is_dark() {
        intensity
    } else {
        0.76 + intensity * 0.19
    }
}

pub(crate) fn window_background(
    theme: Theme,
    appearance: WindowAppearance,
    intensity: f32,
    cx: &App,
) -> Hsla {
    ComponentTheme::global(cx)
        .background
        .alpha(glass_alpha(component_mode(theme, appearance), intensity))
}

pub(crate) fn sync_component_theme(
    theme: Theme,
    applied_mode: &mut Option<ComponentThemeMode>,
    window: &mut Window,
    cx: &mut App,
) {
    let mode = component_mode(theme, window.appearance());
    if *applied_mode == Some(mode) {
        return;
    }

    ComponentTheme::change(mode, Some(window), cx);
    *applied_mode = Some(mode);
}

#[cfg(test)]
mod tests {
    use gpui::{Hsla, Rgba};
    use gpui_component::{Colorize as _, Theme as ComponentTheme, ThemeMode, try_parse_color};

    use super::*;

    fn assert_color(actual: Hsla, expected: &str) {
        let actual: Rgba = actual.into();
        let expected: Rgba = try_parse_color(expected).unwrap().into();
        assert!((actual.r - expected.r).abs() < 0.0001);
        assert!((actual.g - expected.g).abs() < 0.0001);
        assert!((actual.b - expected.b).abs() < 0.0001);
        assert!((actual.a - expected.a).abs() < 0.0001);
    }

    fn linear_channel(channel: f32) -> f32 {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }

    fn luminance(color: Rgba) -> f32 {
        0.2126 * linear_channel(color.r)
            + 0.7152 * linear_channel(color.g)
            + 0.0722 * linear_channel(color.b)
    }

    fn contrast_ratio(first: Rgba, second: Rgba) -> f32 {
        let first = luminance(first);
        let second = luminance(second);
        (first.max(second) + 0.05) / (first.min(second) + 0.05)
    }

    fn composite(foreground: Rgba, background: Rgba) -> Rgba {
        Rgba {
            r: foreground.r * foreground.a + background.r * (1.0 - foreground.a),
            g: foreground.g * foreground.a + background.g * (1.0 - foreground.a),
            b: foreground.b * foreground.a + background.b * (1.0 - foreground.a),
            a: 1.0,
        }
    }

    fn assert_visible_over_light_glass(surface: Rgba) {
        let canvas: Rgba = try_parse_color(LIGHT.canvas).unwrap().into();
        let desktops = [
            Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            Rgba {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        ];

        for intensity in [0.0, 1.0] {
            let canvas = Rgba {
                a: glass_alpha(ThemeMode::Light, intensity),
                ..canvas
            };
            for desktop in desktops {
                let background = composite(canvas, desktop);
                let surface = composite(surface, background);
                assert!(contrast_ratio(background, surface) >= 1.15);
            }
        }
    }

    #[test]
    fn configs_preserve_onechat_glass_and_semantic_colors() {
        for (mode, palette) in [(ThemeMode::Light, LIGHT), (ThemeMode::Dark, DARK)] {
            let config = component_config(mode);
            let colors = &config.colors;
            assert_color(
                try_parse_color(colors.background.as_deref().unwrap()).unwrap(),
                palette.canvas,
            );
            assert_color(
                try_parse_color(colors.sidebar.as_deref().unwrap()).unwrap(),
                palette.sidebar,
            );
            assert_color(
                try_parse_color(colors.popover.as_deref().unwrap()).unwrap(),
                palette.panel,
            );
            assert_color(
                try_parse_color(colors.primary.as_deref().unwrap()).unwrap(),
                palette.accent,
            );
            assert_color(
                try_parse_color(colors.selection.as_deref().unwrap()).unwrap(),
                palette.accent_soft,
            );
            assert_color(
                try_parse_color(colors.secondary.as_deref().unwrap()).unwrap(),
                palette.secondary,
            );
        }
    }

    #[test]
    fn applying_config_refreshes_colors_and_tokens_together() {
        let mut theme = ComponentTheme::default();
        let config = component_config(ThemeMode::Dark);
        theme.apply_config(&config);

        assert_color(theme.background, DARK.canvas);
        assert_color(theme.tokens.background.color, DARK.canvas);
        assert_color(theme.sidebar, DARK.sidebar);
        assert_color(theme.tokens.sidebar.color, DARK.sidebar);
        assert_color(theme.popover, DARK.panel);
        assert_color(theme.tokens.popover.color, DARK.panel);
        assert_color(theme.primary, DARK.accent);
        assert_color(theme.tokens.primary.color, DARK.accent);
        assert_color(theme.selection, DARK.accent_soft);
        assert_color(theme.tokens.selection.color, DARK.accent_soft);
        assert_eq!(theme.font_size, gpui::px(14.0));
        assert_eq!(theme.radius, gpui::px(10.0));
        assert!(theme.shadow);
    }

    #[test]
    fn light_glass_keeps_a_safe_tint_without_becoming_opaque() {
        assert!((glass_alpha(ThemeMode::Light, 0.0) - 0.76).abs() < f32::EPSILON);
        assert!((glass_alpha(ThemeMode::Light, 0.4) - 0.836).abs() < 0.0001);
        assert!((glass_alpha(ThemeMode::Light, 1.0) - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn light_glass_muted_text_stays_readable_over_a_black_desktop() {
        let canvas: Rgba = try_parse_color(LIGHT.canvas).unwrap().into();
        let muted: Rgba = try_parse_color(LIGHT.muted).unwrap().into();
        let alpha = glass_alpha(ThemeMode::Light, 0.0);
        let background = Rgba {
            r: canvas.r * alpha,
            g: canvas.g * alpha,
            b: canvas.b * alpha,
            a: 1.0,
        };

        assert!(contrast_ratio(muted, background) >= 4.5);
    }

    #[test]
    fn light_hover_stays_visible_across_glass_and_desktop_extremes() {
        let hover: Rgba = try_parse_color(LIGHT.hover).unwrap().into();
        assert_visible_over_light_glass(hover);
    }

    #[test]
    fn light_ghost_button_hover_uses_a_visible_secondary_surface() {
        let mut theme = ComponentTheme::default();
        theme.apply_config(&component_config(ThemeMode::Light));
        assert_color(theme.secondary, LIGHT.secondary);

        let ghost_hover: Rgba = theme.secondary.darken(0.1).opacity(0.8).into();
        assert_visible_over_light_glass(ghost_hover);
    }

    #[test]
    fn dark_glass_preserves_the_existing_opacity_behavior() {
        assert!((glass_alpha(ThemeMode::Dark, 0.4) - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn onechat_list_selection_uses_fill_without_component_outline() {
        let mut theme = ComponentTheme::default();
        theme.list.active_highlight = false;
        theme.apply_config(&component_config(ThemeMode::Dark));

        assert!(!theme.list.active_highlight);
        assert_color(theme.list_active, DARK.accent_soft);
    }

    #[test]
    fn font_stack_preserves_primary_and_fallback_order() {
        let font = font_stack(
            &[
                "Maple Mono".into(),
                "LXGW WenKai".into(),
                "PingFang SC".into(),
            ],
            DEFAULT_UI_FONT_FAMILY,
        );

        assert_eq!(font.family.as_ref(), "Maple Mono");
        assert_eq!(
            font.fallbacks.unwrap().fallback_list(),
            &["LXGW WenKai".to_string(), "PingFang SC".to_string()]
        );
    }
}
