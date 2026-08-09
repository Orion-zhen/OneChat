use std::rc::Rc;

use gpui::{
    App, Font, FontFallbacks, Global, Hsla, Rgba, SharedString, Window, WindowAppearance, font,
    rgba,
};
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
    scrim: "#00000088",
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
    scrim: "#00000066",
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
    theme.scrollbar_show = ScrollbarShow::Scrolling;
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

pub(crate) struct UserMessagePalette {
    pub background: Rgba,
    pub foreground: Rgba,
    pub muted_foreground: Rgba,
    pub accent: Rgba,
    pub border: Rgba,
    pub surface: Rgba,
    pub selection: Rgba,
}

pub(crate) fn user_message_palette(cx: &App) -> UserMessagePalette {
    if ComponentTheme::global(cx).is_dark() {
        UserMessagePalette {
            background: rgba(0x1d405cff),
            foreground: rgba(0xf2f7fcff),
            muted_foreground: rgba(0xc0d2e0ff),
            accent: rgba(0x8bd8ffff),
            border: rgba(0x64d2ff2e),
            surface: rgba(0xffffff14),
            selection: rgba(0x64d2ff38),
        }
    } else {
        UserMessagePalette {
            background: rgba(0xe8f3ffff),
            foreground: rgba(0x15324bff),
            muted_foreground: rgba(0x45637dff),
            accent: rgba(0x006ac6ff),
            border: rgba(0x007aff24),
            surface: rgba(0xffffff70),
            selection: rgba(0x007aff2e),
        }
    }
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
