use std::sync::Arc;

use gpui::{
    AnyElement, Div, ElementId, FontWeight, Image, ImageFormat, SharedString, Stateful, div, img,
    prelude::*, px, rgb, rgba,
};

use super::theme::Colors;

pub(crate) fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    button_base(id, colors).child(label.into())
}

pub(crate) fn button_base(id: impl Into<ElementId>, colors: Colors) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_2()
        .rounded_lg()
        .bg(colors.raised)
        .text_sm()
        .cursor_pointer()
        .hover(move |style| style.bg(colors.hover))
        .active(move |style| style.bg(colors.accent_soft))
}

pub(crate) fn primary_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    primary_button_base(id, colors).child(label.into())
}

pub(crate) fn primary_button_base(id: impl Into<ElementId>, colors: Colors) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_2()
        .rounded_lg()
        .bg(colors.accent)
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors.on_accent)
        .cursor_pointer()
        .hover(move |style| {
            style.bg(if colors.dark {
                rgb(0x2693ff)
            } else {
                rgb(0x1683ff)
            })
        })
        .active(move |style| {
            style.bg(if colors.dark {
                rgb(0x0068d6)
            } else {
                rgb(0x006ee6)
            })
        })
}

pub(crate) fn destructive_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_2()
        .rounded_lg()
        .bg(colors.danger)
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors.on_accent)
        .cursor_pointer()
        .hover(move |style| {
            style.bg(if colors.dark {
                rgb(0xff6259)
            } else {
                rgb(0xe31b2e)
            })
        })
        .active(move |style| {
            style.bg(if colors.dark {
                rgb(0xd92f27)
            } else {
                rgb(0xb80012)
            })
        })
        .child(label.into())
}

pub(crate) fn compact_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .px_2()
        .py_1()
        .rounded_md()
        .text_size(px(12.0))
        .cursor_pointer()
        .hover(move |style| style.bg(colors.hover))
        .active(move |style| style.bg(colors.accent_soft))
        .child(label.into())
}

#[derive(Clone, Copy)]
pub(crate) enum UiIcon {
    Copy,
    Pencil,
    Regenerate,
    Info,
    Pin,
    Save,
    Close,
    Settings,
    Menu,
    Plus,
    At,
    Context,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    ChevronUp,
    ArrowUp,
    Stop,
}

#[derive(Clone, Copy)]
pub(crate) enum IconTone {
    Muted,
    Accent,
    Danger,
    OnAccent,
}

pub(crate) fn svg_icon(
    icon: UiIcon,
    tone: IconTone,
    colors: Colors,
    scale_factor: f32,
    size: f32,
) -> AnyElement {
    let display_color = match (tone, colors.dark) {
        (IconTone::Muted, true) => "#a1a1aa",
        (IconTone::Muted, false) => "#6e6e73",
        (IconTone::Accent, true) => "#0a84ff",
        (IconTone::Accent, false) => "#007aff",
        (IconTone::Danger, true) => "#ff453a",
        (IconTone::Danger, false) => "#d70015",
        (IconTone::OnAccent, _) => "#ffffff",
    };
    let image = Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        svg_icon_at_size(icon, &gpui_svg_color(display_color), scale_factor, size).into_bytes(),
    ));
    img(image).size(px(size)).into_any_element()
}

pub(crate) fn svg_icon_button(
    id: impl Into<ElementId>,
    icon: UiIcon,
    tone: IconTone,
    colors: Colors,
    scale_factor: f32,
) -> Stateful<Div> {
    svg_icon_button_sized(id, icon, tone, colors, scale_factor, 24.0, 16.0)
}

pub(crate) fn large_svg_icon_button(
    id: impl Into<ElementId>,
    icon: UiIcon,
    tone: IconTone,
    colors: Colors,
    scale_factor: f32,
) -> Stateful<Div> {
    svg_icon_button_sized(id, icon, tone, colors, scale_factor, 32.0, 20.0)
}

pub(crate) fn primary_svg_icon_button(
    id: impl Into<ElementId>,
    icon: UiIcon,
    colors: Colors,
    scale_factor: f32,
) -> Stateful<Div> {
    svg_icon_button_sized(
        id,
        icon,
        IconTone::OnAccent,
        colors,
        scale_factor,
        32.0,
        20.0,
    )
    .rounded_full()
    .bg(colors.accent)
}

fn svg_icon_button_sized(
    id: impl Into<ElementId>,
    icon: UiIcon,
    tone: IconTone,
    colors: Colors,
    scale_factor: f32,
    button_size: f32,
    icon_size: f32,
) -> Stateful<Div> {
    let (display_color, hover, active) = match (tone, colors.dark) {
        (IconTone::Muted, true) => ("#a1a1aa", colors.hover, colors.accent_soft),
        (IconTone::Muted, false) => ("#6e6e73", colors.hover, colors.accent_soft),
        (IconTone::Accent, true) => ("#0a84ff", colors.hover, colors.accent_soft),
        (IconTone::Accent, false) => ("#007aff", colors.hover, colors.accent_soft),
        (IconTone::Danger, true) => ("#ff453a", rgba(0xff453a18), rgba(0xff453a2e)),
        (IconTone::Danger, false) => ("#d70015", rgba(0xd7001512), rgba(0xd7001524)),
        (IconTone::OnAccent, true) => ("#ffffff", rgb(0x2693ff), rgb(0x0068d6)),
        (IconTone::OnAccent, false) => ("#ffffff", rgb(0x1683ff), rgb(0x006ee6)),
    };
    let svg_color = gpui_svg_color(display_color);
    let image = Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        svg_icon_at_size(icon, &svg_color, scale_factor, icon_size).into_bytes(),
    ));
    div()
        .id(id)
        .size(px(button_size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .active(move |style| style.bg(active))
        .child(img(image).size(px(icon_size)))
}

fn gpui_svg_color(display_color: &str) -> String {
    // GPUI 0.2.2 uploads SVG RGBA pixels as BGRA, so compensate before rasterization.
    format!(
        "#{}{}{}",
        &display_color[5..7],
        &display_color[3..5],
        &display_color[1..3]
    )
}

fn svg_icon_at_size(icon: UiIcon, color: &str, scale_factor: f32, size: f32) -> String {
    let paths = match icon {
        UiIcon::Copy => {
            r#"<rect width="13" height="13" x="9" y="9" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>"#
        }
        UiIcon::Pencil => {
            r#"<path d="M12 20h9"/><path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L8 18l-4 1 1-4Z"/><path d="m15 5 3 3"/>"#
        }
        UiIcon::Regenerate => {
            r#"<path d="M20 11a8.1 8.1 0 0 0-14.5-4.9L3 9"/><path d="M3 4v5h5"/><path d="M4 13a8.1 8.1 0 0 0 14.5 4.9L21 15"/><path d="M16 15h5v5"/>"#
        }
        UiIcon::Info => {
            r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v5"/><path d="M12 8h.01"/>"#
        }
        UiIcon::Pin => {
            r#"<path d="M12 17v5"/><path d="M5 17h14"/><path d="M6 17h12l-1-5 2-2V8H5v2l2 2Z"/><path d="M9 8V2h6v6"/>"#
        }
        UiIcon::Save => {
            r#"<path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Z"/><path d="M17 21v-8H7v8"/><path d="M7 3v5h8"/>"#
        }
        UiIcon::Close => r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#,
        UiIcon::Settings => {
            r#"<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.09a2 2 0 0 1 1 1.74v.5a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.38a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2Z"/><circle cx="12" cy="12" r="3"/>"#
        }
        UiIcon::Menu => r#"<path d="M3 6h18"/><path d="M3 12h18"/><path d="M3 18h18"/>"#,
        UiIcon::Plus => r#"<path d="M12 3v18"/><path d="M3 12h18"/>"#,
        UiIcon::At => {
            r#"<circle cx="12" cy="12" r="4"/><path d="M16 8v5a3 3 0 0 0 6 0v-1a10 10 0 1 0-4 8"/>"#
        }
        UiIcon::Context => {
            r#"<circle cx="11" cy="12.25" r="8"/><path d="m7 12.25 3.5 3.5L20.5 3.75"/>"#
        }
        UiIcon::ChevronLeft => r#"<path d="m16 20-8-8 8-8"/>"#,
        UiIcon::ChevronRight => r#"<path d="m8 4 8 8-8 8"/>"#,
        UiIcon::ChevronDown => r#"<path d="m6 9 6 6 6-6"/>"#,
        UiIcon::ChevronUp => r#"<path d="m18 15-6-6-6 6"/>"#,
        UiIcon::ArrowUp => r#"<path d="M12 19V5"/><path d="m5 12 7-7 7 7"/>"#,
        UiIcon::Stop => {
            r#"<rect width="10" height="10" x="7" y="7" rx="1.5" fill="currentColor" stroke="none"/>"#
        }
    };
    let physical_size = (size * scale_factor.max(1.0)).round() as u32;
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{physical_size}" height="{physical_size}" viewBox="0 0 24 24" color="{color}" fill="none" stroke="{color}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" shape-rendering="geometricPrecision">{paths}</svg>"#
    )
}

pub(crate) fn icon_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(32.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .cursor_pointer()
        .hover(move |style| style.bg(colors.hover))
        .active(move |style| style.bg(colors.accent_soft))
        .child(label.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_icons_rasterize_at_the_display_scale() {
        let svg = svg_icon_at_size(UiIcon::Pin, "#ffffff", 2.0, 16.0);

        assert!(svg.contains(r#"width="32" height="32""#));
        assert!(svg.contains(r#"viewBox="0 0 24 24""#));
        assert!(svg.contains(r#"shape-rendering="geometricPrecision""#));

        assert_eq!(gpui_svg_color("#ff453a"), "#3a45ff");
        assert_eq!(gpui_svg_color("#0a84ff"), "#ff840a");

        let close = svg_icon_at_size(UiIcon::Close, &gpui_svg_color("#d70015"), 2.0, 16.0);
        assert!(close.contains(r##"stroke="#1500d7""##));
        assert!(close.contains(r#"<path d="M18 6 6 18"/>"#));

        let settings = svg_icon_at_size(UiIcon::Settings, "#ffffff", 2.0, 20.0);
        assert!(settings.contains(r#"width="40" height="40""#));
    }
}
