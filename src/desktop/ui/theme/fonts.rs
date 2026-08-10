use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AppFonts {
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

pub(super) fn app_fonts(ui_families: &[String], code_families: &[String]) -> AppFonts {
    AppFonts {
        ui: font_stack(ui_families, DEFAULT_UI_FONT_FAMILY),
        code: font_stack(code_families, DEFAULT_CODE_FONT_FAMILY),
    }
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
