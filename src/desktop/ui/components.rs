use gpui::{Div, ElementId, FontWeight, SharedString, Stateful, div, prelude::*, px, rgb, rgba};

use super::{
    icons::{Icon, IconTone, render_icon},
    theme::Colors,
};

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

pub(crate) fn icon_button(
    id: impl Into<ElementId>,
    icon_name: Icon,
    tone: IconTone,
    colors: Colors,
    scale_factor: f32,
) -> Stateful<Div> {
    icon_button_sized(id, icon_name, tone, colors, scale_factor, 24.0, 16.0)
}

pub(crate) fn large_icon_button(
    id: impl Into<ElementId>,
    icon_name: Icon,
    tone: IconTone,
    colors: Colors,
    scale_factor: f32,
) -> Stateful<Div> {
    icon_button_sized(id, icon_name, tone, colors, scale_factor, 32.0, 20.0)
}

pub(crate) fn primary_icon_button(
    id: impl Into<ElementId>,
    icon_name: Icon,
    colors: Colors,
    scale_factor: f32,
) -> Stateful<Div> {
    icon_button_sized(
        id,
        icon_name,
        IconTone::OnAccent,
        colors,
        scale_factor,
        32.0,
        20.0,
    )
    .rounded_full()
    .bg(colors.accent)
}

fn icon_button_sized(
    id: impl Into<ElementId>,
    icon_name: Icon,
    tone: IconTone,
    colors: Colors,
    scale_factor: f32,
    button_size: f32,
    icon_size: f32,
) -> Stateful<Div> {
    let (hover, active) = match (tone, colors.dark) {
        (IconTone::Muted | IconTone::Accent, _) => (colors.hover, colors.accent_soft),
        (IconTone::Danger, true) => (rgba(0xff453a18), rgba(0xff453a2e)),
        (IconTone::Danger, false) => (rgba(0xd7001512), rgba(0xd7001524)),
        (IconTone::OnAccent, true) => (rgb(0x2693ff), rgb(0x0068d6)),
        (IconTone::OnAccent, false) => (rgb(0x1683ff), rgb(0x006ee6)),
    };
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
        .child(render_icon(
            icon_name,
            tone,
            colors,
            scale_factor,
            icon_size,
        ))
}
