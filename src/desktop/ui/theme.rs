use std::rc::Rc;

use gpui::{App, SharedString, Window, WindowAppearance};
use gpui_component::{
    Theme as ComponentTheme, ThemeConfig, ThemeConfigColors, ThemeMode as ComponentThemeMode,
};

use crate::domain::Theme;

#[derive(Clone, Copy)]
struct Palette {
    canvas: &'static str,
    sidebar: &'static str,
    toolbar: &'static str,
    panel: &'static str,
    raised: &'static str,
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
    raised: "#76768012",
    hover: "#7676801A",
    text: "#1D1D1F",
    muted: "#6E6E73",
    border: "#3C3C4318",
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
        secondary: palette.raised,
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
        mono_font_family: Some(if cfg!(target_os = "macos") {
            "Menlo".into()
        } else if cfg!(target_os = "windows") {
            "Consolas".into()
        } else {
            "DejaVu Sans Mono".into()
        }),
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
}

pub(crate) fn component_mode(theme: Theme, appearance: WindowAppearance) -> ComponentThemeMode {
    match theme {
        Theme::Light => ComponentThemeMode::Light,
        Theme::Dark => ComponentThemeMode::Dark,
        Theme::System => appearance.into(),
    }
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
    use gpui_component::{Theme as ComponentTheme, ThemeMode, try_parse_color};

    use super::*;

    fn assert_color(actual: Hsla, expected: &str) {
        let actual: Rgba = actual.into();
        let expected: Rgba = try_parse_color(expected).unwrap().into();
        assert!((actual.r - expected.r).abs() < 0.0001);
        assert!((actual.g - expected.g).abs() < 0.0001);
        assert!((actual.b - expected.b).abs() < 0.0001);
        assert!((actual.a - expected.a).abs() < 0.0001);
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
    fn onechat_list_selection_uses_fill_without_component_outline() {
        let mut theme = ComponentTheme::default();
        theme.list.active_highlight = false;
        theme.apply_config(&component_config(ThemeMode::Dark));

        assert!(!theme.list.active_highlight);
        assert_color(theme.list_active, DARK.accent_soft);
    }
}
