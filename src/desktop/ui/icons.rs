use std::sync::Arc;

use gpui::{AnyElement, Image, ImageFormat, img, prelude::*, px};

use super::theme::Colors;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Icon {
    AlertTriangle,
    ArrowUp,
    At,
    Check,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    Close,
    ContextSelect,
    ContextSelected,
    Copy,
    Eye,
    Fork,
    Info,
    Layers,
    Menu,
    MessageText,
    Monitor,
    Moon,
    Pencil,
    Pin,
    Plus,
    Regenerate,
    Save,
    Settings,
    Sliders,
    Stop,
    Sun,
    Trash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IconTone {
    Muted,
    Accent,
    Danger,
    OnAccent,
}

pub(crate) fn render_icon(
    icon: Icon,
    tone: IconTone,
    colors: Colors,
    scale_factor: f32,
    size: f32,
) -> AnyElement {
    let color = gpui_svg_color(icon_color(tone, colors));
    let image = Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        icon_svg(icon, &color, scale_factor, size).into_bytes(),
    ));
    img(image).size(px(size)).into_any_element()
}

fn icon_color(tone: IconTone, colors: Colors) -> &'static str {
    match (tone, colors.dark) {
        (IconTone::Muted, true) => "#a1a1aa",
        (IconTone::Muted, false) => "#6e6e73",
        (IconTone::Accent, true) => "#0a84ff",
        (IconTone::Accent, false) => "#007aff",
        (IconTone::Danger, true) => "#ff453a",
        (IconTone::Danger, false) => "#d70015",
        (IconTone::OnAccent, _) => "#ffffff",
    }
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

fn icon_svg(icon: Icon, color: &str, scale_factor: f32, size: f32) -> String {
    let paths = match icon {
        Icon::AlertTriangle => {
            r#"<path d="M10.3 2.9 1.8 17a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 2.9a2 2 0 0 0-3.4 0Z"/><path d="M12 9v4"/><path d="M12 17h.01"/>"#
        }
        Icon::ArrowUp => r#"<path d="M12 19V5"/><path d="m5 12 7-7 7 7"/>"#,
        Icon::At => {
            r#"<circle cx="12" cy="12" r="4"/><path d="M16 8v5a3 3 0 0 0 6 0v-1a10 10 0 1 0-4 8"/>"#
        }
        Icon::Check => r#"<path d="m20 6-11 11-5-5"/>"#,
        Icon::ChevronDown => r#"<path d="m6 9 6 6 6-6"/>"#,
        Icon::ChevronLeft => r#"<path d="m16 20-8-8 8-8"/>"#,
        Icon::ChevronRight => r#"<path d="m8 4 8 8-8 8"/>"#,
        Icon::ChevronUp => r#"<path d="m18 15-6-6-6 6"/>"#,
        Icon::Close => r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#,
        Icon::ContextSelect => {
            r#"<path d="M21 15a4 4 0 0 1-4 4H8l-5 3V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4Z"/><path d="m8.5 11 2.25 2.25 4.75-5" stroke-width="2"/>"#
        }
        Icon::ContextSelected => {
            r##"<circle cx="12" cy="12" r="9" fill="currentColor"/><path d="m8 12 2.75 2.75L16.5 9" stroke="#ffffff" stroke-width="2.2"/>"##
        }
        Icon::Copy => {
            r#"<rect width="13" height="13" x="9" y="9" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>"#
        }
        Icon::Eye => {
            r#"<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z"/><circle cx="12" cy="12" r="3"/>"#
        }
        Icon::Fork => {
            r#"<circle cx="12" cy="18" r="3"/><circle cx="6" cy="6" r="3"/><circle cx="18" cy="6" r="3"/><path d="M6 9a6 6 0 0 0 6 6"/><path d="M18 9a6 6 0 0 1-6 6"/>"#
        }
        Icon::Info => r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v5"/><path d="M12 8h.01"/>"#,
        Icon::Layers => {
            r#"<path d="m12 2 9 5-9 5-9-5 9-5Z"/><path d="m3 12 9 5 9-5"/><path d="m3 17 9 5 9-5"/>"#
        }
        Icon::Menu => r#"<path d="M3 6h18"/><path d="M3 12h18"/><path d="M3 18h18"/>"#,
        Icon::MessageText => {
            r#"<path d="M21 15a4 4 0 0 1-4 4H8l-5 3V7a4 4 0 0 1 4-4h10a4 4 0 0 1 4 4Z"/><path d="M8 8h8"/><path d="M8 12h6"/>"#
        }
        Icon::Monitor => {
            r#"<rect width="18" height="14" x="3" y="3" rx="2"/><path d="M8 21h8"/><path d="M12 17v4"/>"#
        }
        Icon::Moon => r#"<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/>"#,
        Icon::Pencil => {
            r#"<path d="M12 20h9"/><path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L8 18l-4 1 1-4Z"/><path d="m15 5 3 3"/>"#
        }
        Icon::Pin => {
            r#"<path d="M12 17v5"/><path d="M5 17h14"/><path d="M6 17h12l-1-5 2-2V8H5v2l2 2Z"/><path d="M9 8V2h6v6"/>"#
        }
        Icon::Plus => r#"<path d="M12 3v18"/><path d="M3 12h18"/>"#,
        Icon::Regenerate => {
            r#"<path d="M20 11a8.1 8.1 0 0 0-14.5-4.9L3 9"/><path d="M3 4v5h5"/><path d="M4 13a8.1 8.1 0 0 0 14.5 4.9L21 15"/><path d="M16 15h5v5"/>"#
        }
        Icon::Save => {
            r#"<path d="M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Z"/><path d="M17 21v-8H7v8"/><path d="M7 3v5h8"/>"#
        }
        Icon::Settings => {
            r#"<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.09a2 2 0 0 1 1 1.74v.5a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.38a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2Z"/><circle cx="12" cy="12" r="3"/>"#
        }
        Icon::Sliders => {
            r#"<path d="M4 21v-7"/><path d="M4 10V3"/><path d="M12 21v-9"/><path d="M12 8V3"/><path d="M20 21v-5"/><path d="M20 12V3"/><path d="M1 14h6"/><path d="M9 8h6"/><path d="M17 16h6"/>"#
        }
        Icon::Stop => {
            r#"<rect width="12" height="12" x="6" y="6" rx="2" fill="currentColor" stroke="none"/>"#
        }
        Icon::Sun => {
            r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2"/><path d="M12 20v2"/><path d="m4.93 4.93 1.42 1.42"/><path d="m17.66 17.66 1.41 1.41"/><path d="M2 12h2"/><path d="M20 12h2"/><path d="m6.34 17.66-1.41 1.41"/><path d="m19.07 4.93-1.41 1.41"/>"#
        }
        Icon::Trash => {
            r#"<path d="M3 6h18"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/><path d="M10 11v6"/><path d="M14 11v6"/>"#
        }
    };
    let physical_size = (size * scale_factor.max(1.0)).round() as u32;
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{physical_size}" height="{physical_size}" viewBox="0 0 24 24" color="{color}" fill="none" stroke="{color}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" shape-rendering="geometricPrecision">{paths}</svg>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_rasterize_at_the_display_scale() {
        let svg = icon_svg(Icon::Pin, "#ffffff", 2.0, 16.0);

        assert!(svg.contains(r#"width="32" height="32""#));
        assert!(svg.contains(r#"viewBox="0 0 24 24""#));
        assert!(svg.contains(r#"shape-rendering="geometricPrecision""#));

        assert_eq!(gpui_svg_color("#ff453a"), "#3a45ff");
        assert_eq!(gpui_svg_color("#0a84ff"), "#ff840a");

        let close = icon_svg(Icon::Close, &gpui_svg_color("#d70015"), 2.0, 16.0);
        assert!(close.contains(r##"stroke="#1500d7""##));
        assert!(close.contains(r#"<path d="M18 6 6 18"/>"#));

        let check = icon_svg(Icon::Check, "#ffffff", 2.0, 14.0);
        assert!(check.contains(r#"width="28" height="28""#));

        let stop = icon_svg(Icon::Stop, "#ffffff", 2.0, 18.0);
        assert!(stop.contains(
            r#"<rect width="12" height="12" x="6" y="6" rx="2" fill="currentColor" stroke="none"/>"#
        ));
    }
}
