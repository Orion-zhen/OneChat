use super::*;

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
    theme.scrollbar_mode = ScrollbarMode::Scrolling;
    cx.set_global(ActiveAppPalette(AppPalette::generate(initial_mode, seed)));
    cx.set_global(app_fonts(&[], &[]));
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
