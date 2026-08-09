use std::rc::Rc;

use gpui::{
    App, Font, FontFallbacks, Global, Hsla, Rgba, SharedString, Window, WindowAppearance, font,
    rgba,
};
use gpui_component::{
    Colorize as _, Theme as ComponentTheme, ThemeConfig, ThemeConfigColors,
    ThemeMode as ComponentThemeMode, scroll::ScrollbarShow,
};

use crate::domain::{
    DEFAULT_CODE_FONT_FAMILY, DEFAULT_THEME_COLOR, DEFAULT_UI_FONT_FAMILY, Theme,
    normalize_font_families,
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct UserMessagePalette {
    pub background: Hsla,
    pub foreground: Hsla,
    pub muted_foreground: Hsla,
    pub link: Hsla,
    pub emphasis: Hsla,
    pub border: Hsla,
    pub surface: Hsla,
    pub selection: Hsla,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AppPalette {
    pub canvas: Hsla,
    pub sidebar: Hsla,
    pub toolbar: Hsla,
    pub panel: Hsla,
    pub overlay_panel: Hsla,
    pub raised: Hsla,
    pub secondary: Hsla,
    pub secondary_hover: Hsla,
    pub secondary_active: Hsla,
    pub hover: Hsla,
    pub foreground: Hsla,
    pub muted_foreground: Hsla,
    pub border: Hsla,
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub accent_active: Hsla,
    pub accent_soft: Hsla,
    pub accent_border: Hsla,
    pub on_accent: Hsla,
    pub link: Hsla,
    pub emphasis: Hsla,
    pub selection: Hsla,
    pub danger: Hsla,
    pub danger_hover: Hsla,
    pub danger_active: Hsla,
    pub danger_soft: Hsla,
    pub danger_subtle: Hsla,
    pub on_danger: Hsla,
    pub success: Hsla,
    pub success_hover: Hsla,
    pub success_active: Hsla,
    pub on_success: Hsla,
    pub control_thumb: Hsla,
    pub scrim: Hsla,
    pub floating_glass: Hsla,
    pub floating_border: Hsla,
    pub floating_shadow: Hsla,
    pub media_border: Hsla,
    pub document_background: Hsla,
    pub document_border: Hsla,
    pub user_message: UserMessagePalette,
}

#[derive(Clone, Copy)]
struct BasePalette {
    canvas: Hsla,
    sidebar: Hsla,
    toolbar: Hsla,
    panel: Hsla,
    raised: Hsla,
    secondary: Hsla,
    secondary_hover: Hsla,
    secondary_active: Hsla,
    hover: Hsla,
    foreground: Hsla,
    muted_foreground: Hsla,
    border: Hsla,
    scrim: Hsla,
}

#[derive(Clone, Copy)]
struct ActiveAppPalette(AppPalette);

impl Global for ActiveAppPalette {}

pub(crate) fn palette(cx: &App) -> &AppPalette {
    &cx.global::<ActiveAppPalette>().0
}

fn opaque(mut color: Hsla) -> Hsla {
    color.a = 1.0;
    color
}

pub(crate) fn parse_theme_color(value: &str) -> Hsla {
    Hsla::parse_hex(value)
        .map(opaque)
        .unwrap_or_else(|_| Hsla::parse_hex(DEFAULT_THEME_COLOR).expect("default theme color"))
}

fn mix_toward(color: Hsla, target: Hsla, amount: f32) -> Hsla {
    color.mix_oklab(target, 1.0 - amount.clamp(0.0, 1.0))
}

fn tinted(base: Hsla, tint: Hsla, amount: f32) -> Hsla {
    let mut result = mix_toward(opaque(base), opaque(tint), amount);
    result.a = base.a;
    result
}

fn relative_luminance(color: Hsla) -> f32 {
    let color: Rgba = color.into();
    let linear = |channel: f32| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

fn contrast_ratio(a: Hsla, b: Hsla) -> f32 {
    let (lighter, darker) = {
        let a = relative_luminance(a);
        let b = relative_luminance(b);
        if a > b { (a, b) } else { (b, a) }
    };
    (lighter + 0.05) / (darker + 0.05)
}

fn contrasting_foreground(background: Hsla) -> Hsla {
    let black = Hsla::black();
    let white = Hsla::white();
    if contrast_ratio(black, background) >= contrast_ratio(white, background) {
        black
    } else {
        white
    }
}

fn readable_accent(seed: Hsla, background: Hsla, dark: bool) -> Hsla {
    let target = if dark { Hsla::white() } else { Hsla::black() };
    (0..=20)
        .map(|step| mix_toward(seed, target, step as f32 * 0.05))
        .find(|accent| contrast_ratio(*accent, background) >= 4.5)
        .unwrap_or(target)
}

fn accent_interaction_colors(accent: Hsla) -> (Hsla, Hsla) {
    (
        mix_toward(accent, Hsla::white(), 0.07),
        mix_toward(accent, Hsla::black(), 0.1),
    )
}

fn base_palette(dark: bool) -> BasePalette {
    if dark {
        BasePalette {
            canvas: rgba(0x18181af2).into(),
            sidebar: rgba(0x242426dc).into(),
            toolbar: rgba(0x1d1d1fdc).into(),
            panel: rgba(0x2c2c2eec).into(),
            raised: rgba(0xffffff10).into(),
            secondary: rgba(0xffffff10).into(),
            secondary_hover: rgba(0xffffff18).into(),
            secondary_active: rgba(0xffffff26).into(),
            hover: rgba(0xffffff18).into(),
            foreground: rgba(0xf5f5f7ff).into(),
            muted_foreground: rgba(0xa1a1aaff).into(),
            border: rgba(0xffffff16).into(),
            scrim: rgba(0x00000088).into(),
        }
    } else {
        BasePalette {
            canvas: rgba(0xf5f5f7f2).into(),
            sidebar: rgba(0xebebf0dc).into(),
            toolbar: rgba(0xf7f7f8dc).into(),
            panel: rgba(0xffffffec).into(),
            raised: rgba(0xffffff52).into(),
            secondary: rgba(0x3c3c4324).into(),
            secondary_hover: rgba(0x3c3c4330).into(),
            secondary_active: rgba(0x3c3c4340).into(),
            hover: rgba(0x3c3c4330).into(),
            foreground: rgba(0x1d1d1fff).into(),
            muted_foreground: rgba(0x48484aff).into(),
            border: rgba(0x3c3c4330).into(),
            scrim: rgba(0x00000066).into(),
        }
    }
}

impl AppPalette {
    fn generate(mode: ComponentThemeMode, seed: Hsla) -> Self {
        let dark = mode.is_dark();
        let seed = opaque(seed);
        let BasePalette {
            canvas,
            sidebar,
            toolbar,
            panel,
            raised,
            secondary,
            secondary_hover,
            secondary_active,
            hover,
            foreground,
            muted_foreground,
            border,
            scrim,
        } = base_palette(dark);
        let surface_tint = if dark { 0.025 } else { 0.012 };
        let canvas = tinted(canvas, seed, surface_tint * 0.5);
        let sidebar = tinted(sidebar, seed, surface_tint);
        let toolbar = tinted(toolbar, seed, surface_tint * 0.7);
        let panel = tinted(panel, seed, surface_tint);
        let accent = readable_accent(seed, canvas, dark);
        let (accent_hover, accent_active) = accent_interaction_colors(accent);
        let accent_soft = accent.alpha(if dark { 0.18 } else { 0.12 });
        let link = readable_accent(seed, canvas, dark);
        let emphasis = mix_toward(foreground, link, if dark { 0.3 } else { 0.22 });
        let selection = accent.alpha(if dark { 0.32 } else { 0.22 });
        let on_accent = contrasting_foreground(accent);

        let danger: Hsla = if dark {
            rgba(0xff453aff).into()
        } else {
            rgba(0xd70015ff).into()
        };
        let success: Hsla = if dark {
            rgba(0x30d158ff).into()
        } else {
            rgba(0x248a3dff).into()
        };
        let (danger_hover, danger_active) = accent_interaction_colors(danger);
        let (success_hover, success_active) = accent_interaction_colors(success);
        let danger_soft = danger.alpha(if dark { 0.14 } else { 0.095 });
        let danger_subtle = danger.alpha(if dark { 0.085 } else { 0.05 });

        let user_background = tinted(panel, accent, if dark { 0.22 } else { 0.09 });
        let user_foreground = foreground;
        let user_link = readable_accent(seed, user_background, dark);
        let user_message = UserMessagePalette {
            background: user_background,
            foreground: user_foreground,
            muted_foreground: if dark {
                tinted(muted_foreground, accent, 0.08)
            } else {
                tinted(muted_foreground, accent, 0.06)
            },
            link: user_link,
            emphasis: mix_toward(user_foreground, user_link, if dark { 0.35 } else { 0.24 }),
            border: accent.alpha(if dark { 0.18 } else { 0.14 }),
            surface: Hsla::white().alpha(if dark { 0.08 } else { 0.44 }),
            selection: accent.alpha(if dark { 0.22 } else { 0.18 }),
        };

        Self {
            canvas,
            sidebar,
            toolbar,
            panel,
            overlay_panel: panel.alpha(0.95),
            raised,
            secondary,
            secondary_hover,
            secondary_active,
            hover,
            foreground,
            muted_foreground,
            border,
            accent,
            accent_hover,
            accent_active,
            accent_soft,
            accent_border: accent.alpha(0.35),
            on_accent,
            link,
            emphasis,
            selection,
            danger,
            danger_hover,
            danger_active,
            danger_soft,
            danger_subtle,
            on_danger: contrasting_foreground(danger),
            success,
            success_hover,
            success_active,
            on_success: contrasting_foreground(success),
            control_thumb: Hsla::white(),
            scrim,
            floating_glass: if dark {
                rgba(0x2c2c2ef2).into()
            } else {
                rgba(0xfffffff2).into()
            },
            floating_border: if dark {
                rgba(0xffffff38).into()
            } else {
                rgba(0x3c3c4326).into()
            },
            floating_shadow: if dark {
                rgba(0x0000005c).into()
            } else {
                rgba(0x1d1d1f24).into()
            },
            media_border: if dark {
                rgba(0xffffff26).into()
            } else {
                rgba(0x0000001f).into()
            },
            document_background: rgba(0xffffffff).into(),
            document_border: rgba(0x0000001f).into(),
            user_message,
        }
    }
}

fn color(value: Hsla) -> Option<SharedString> {
    Some(value.to_hex().into())
}

fn component_config(mode: ComponentThemeMode, seed: Hsla) -> Rc<ThemeConfig> {
    let palette = AppPalette::generate(mode, seed);
    let mut colors = ThemeConfigColors::default();
    macro_rules! set_colors {
        ($($field:ident: $value:expr),+ $(,)?) => {
            $(colors.$field = color($value);)+
        };
    }
    set_colors! {
        background: palette.canvas,
        foreground: palette.foreground,
        border: palette.border,
        input: palette.border,
        muted: palette.raised,
        muted_foreground: palette.muted_foreground,
        accent: palette.accent_soft,
        accent_foreground: palette.foreground,
        primary: palette.accent,
        primary_active: palette.accent_active,
        primary_hover: palette.accent_hover,
        primary_foreground: palette.on_accent,
        secondary: palette.secondary,
        secondary_active: palette.secondary_active,
        secondary_hover: palette.secondary_hover,
        secondary_foreground: palette.foreground,
        danger: palette.danger,
        danger_active: palette.danger_active,
        danger_foreground: palette.on_danger,
        danger_hover: palette.danger_hover,
        success: palette.success,
        success_active: palette.success_active,
        success_foreground: palette.on_success,
        success_hover: palette.success_hover,
        button: palette.raised,
        button_active: palette.secondary_active,
        button_foreground: palette.foreground,
        button_hover: palette.secondary_hover,
        button_primary: palette.accent,
        button_primary_active: palette.accent_active,
        button_primary_foreground: palette.on_accent,
        button_primary_hover: palette.accent_hover,
        button_secondary: palette.secondary,
        button_secondary_active: palette.secondary_active,
        button_secondary_foreground: palette.foreground,
        button_secondary_hover: palette.secondary_hover,
        button_danger: palette.danger,
        button_danger_active: palette.danger_active,
        button_danger_foreground: palette.on_danger,
        button_danger_hover: palette.danger_hover,
        button_success: palette.success,
        button_success_active: palette.success_active,
        button_success_foreground: palette.on_success,
        button_success_hover: palette.success_hover,
        caret: palette.accent,
        ring: palette.accent,
        selection: palette.selection,
        list: palette.panel,
        list_active: palette.accent_soft,
        list_active_border: palette.accent,
        list_even: palette.panel,
        list_head: palette.panel,
        list_hover: palette.hover,
        popover: palette.panel,
        popover_foreground: palette.foreground,
        sidebar: palette.sidebar,
        sidebar_accent: palette.accent_soft,
        sidebar_accent_foreground: palette.foreground,
        sidebar_border: palette.border,
        sidebar_foreground: palette.foreground,
        sidebar_primary: palette.accent,
        sidebar_primary_foreground: palette.on_accent,
        title_bar: palette.toolbar,
        title_bar_border: palette.border,
        overlay: palette.scrim,
        group_box: palette.panel,
        group_box_foreground: palette.foreground,
        slider_bar: palette.accent,
        slider_thumb: palette.control_thumb,
        switch: palette.secondary_active,
        switch_thumb: palette.control_thumb,
        tab: palette.panel,
        tab_active: palette.panel,
        tab_active_foreground: palette.foreground,
        tab_bar: palette.toolbar,
        tab_bar_segmented: palette.raised,
        tab_foreground: palette.muted_foreground,
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
    let seed = parse_theme_color(DEFAULT_THEME_COLOR);
    let light = component_config(ComponentThemeMode::Light, seed);
    let dark = component_config(ComponentThemeMode::Dark, seed);
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
    theme.scrollbar_show = ScrollbarShow::Scrolling;
    cx.set_global(ActiveAppPalette(AppPalette::generate(initial_mode, seed)));
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
    palette(cx)
        .canvas
        .alpha(glass_alpha(component_mode(theme, appearance), intensity))
}

pub(crate) fn sync_component_theme(
    theme: Theme,
    theme_color: &str,
    applied_theme: &mut Option<(ComponentThemeMode, String)>,
    window: &mut Window,
    cx: &mut App,
) {
    let mode = component_mode(theme, window.appearance());
    let seed = parse_theme_color(theme_color);
    let normalized_color = seed.to_hex();
    if applied_theme
        .as_ref()
        .is_some_and(|(applied_mode, applied_color)| {
            *applied_mode == mode && applied_color == &normalized_color
        })
    {
        return;
    }

    let light = component_config(ComponentThemeMode::Light, seed);
    let dark = component_config(ComponentThemeMode::Dark, seed);
    {
        let component_theme = ComponentTheme::global_mut(cx);
        component_theme.apply_config(&light);
        component_theme.apply_config(&dark);
    }
    ComponentTheme::change(mode, Some(window), cx);
    *cx.global_mut::<ActiveAppPalette>() = ActiveAppPalette(AppPalette::generate(mode, seed));
    *applied_theme = Some((mode, normalized_color.to_string()));
}

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
