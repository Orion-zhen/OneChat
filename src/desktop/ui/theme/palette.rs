use super::*;

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
    pub scrollbar_thumb: Hsla,
    pub scrollbar_thumb_hover: Hsla,
    pub scrollbar_thumb_active: Hsla,
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
pub(super) struct ActiveAppPalette(pub(super) AppPalette);

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

pub(super) fn contrast_ratio(a: Hsla, b: Hsla) -> f32 {
    let (lighter, darker) = {
        let a = relative_luminance(a);
        let b = relative_luminance(b);
        if a > b { (a, b) } else { (b, a) }
    };
    (lighter + 0.05) / (darker + 0.05)
}

fn subtle_contrasting_control(target: Hsla, background: Hsla, minimum: f32) -> Hsla {
    (0..=20)
        .map(|step| mix_toward(background, target, step as f32 * 0.05))
        .find(|color| contrast_ratio(*color, background) >= minimum)
        .unwrap_or(target)
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
    pub(super) fn generate(mode: ComponentThemeMode, seed: Hsla) -> Self {
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
        let scrollbar_target = tinted(foreground, accent, if dark { 0.14 } else { 0.1 });
        let scrollbar_background = opaque(user_background);
        let scrollbar_thumb =
            subtle_contrasting_control(scrollbar_target, scrollbar_background, 3.0);
        let scrollbar_thumb_hover =
            subtle_contrasting_control(scrollbar_target, scrollbar_background, 4.5);
        let scrollbar_thumb_active =
            subtle_contrasting_control(scrollbar_target, scrollbar_background, 6.0);

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
            scrollbar_thumb,
            scrollbar_thumb_hover,
            scrollbar_thumb_active,
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
