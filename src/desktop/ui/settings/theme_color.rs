use gpui::{Anchor, Focusable as _, Hsla, linear_color_stop, linear_gradient, px};
use gpui_component::{
    Colorize as _,
    input::{Input, InputEvent, InputState},
    popover::Popover,
    slider::{Slider, SliderEvent, SliderState},
};

use super::*;

const PRESET_COLORS: &[&str] = &[
    "#8E8E93", "#FF3B30", "#FF9500", "#FFCC00", "#34C759", "#00C7BE", "#30B0C7", "#32ADE6",
    "#007AFF", "#5856D6", "#AF52DE", "#FF2D55",
];

pub(crate) struct ThemeColorControl {
    hue: Entity<SliderState>,
    saturation: Entity<SliderState>,
    lightness: Entity<SliderState>,
    hex: Entity<InputState>,
}

impl ThemeColorControl {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<OneChat>) -> Self {
        let color =
            crate::desktop::ui::theme::parse_theme_color(crate::domain::DEFAULT_THEME_COLOR);
        let hue = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(1.0)
                .step(0.001)
                .default_value(color.h)
        });
        let saturation = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(1.0)
                .step(0.01)
                .default_value(color.s)
        });
        let lightness = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(1.0)
                .step(0.01)
                .default_value(color.l)
        });
        let hex = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(crate::domain::DEFAULT_THEME_COLOR)
                .placeholder("#007AFF")
        });

        for slider in [&hue, &saturation, &lightness] {
            cx.subscribe(slider, |this, _, _: &SliderEvent, cx| {
                this.update_theme_color_from_controls(cx);
            })
            .detach();
        }
        cx.subscribe(&hex, |this, input, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::Change | InputEvent::PressEnter { .. }) {
                return;
            }
            let value = input.read(cx).value();
            if value.len() == 7 && Hsla::parse_hex(&value).is_ok() {
                this.set_theme_color(value.to_string(), cx);
            }
        })
        .detach();

        Self {
            hue,
            saturation,
            lightness,
            hex,
        }
    }

    pub(crate) fn color(&self, cx: &App) -> Hsla {
        gpui::hsla(
            self.hue.read(cx).value().start(),
            self.saturation.read(cx).value().start(),
            self.lightness.read(cx).value().start(),
            1.0,
        )
    }

    pub(crate) fn sync(&self, color: Hsla, window: &mut Window, cx: &mut App) {
        sync_slider(&self.hue, color.h, window, cx);
        sync_slider(&self.saturation, color.s, window, cx);
        sync_slider(&self.lightness, color.l, window, cx);

        let value = color.to_hex();
        let input = self.hex.read(cx);
        if !input.focus_handle(cx).is_focused(window) && input.value().as_ref() != value {
            self.hex
                .update(cx, |input, cx| input.set_value(value, window, cx));
        }
    }
}

fn sync_slider(state: &Entity<SliderState>, value: f32, window: &mut Window, cx: &mut App) {
    if (state.read(cx).value().start() - value).abs() > 0.005 {
        state.update(cx, |slider, cx| slider.set_value(value, window, cx));
    }
}

pub(in crate::desktop::ui::settings) fn theme_color_picker(
    app: &OneChat,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let color = crate::desktop::ui::theme::parse_theme_color(&app.settings().theme_color);
    let hex = color.to_hex();
    let trigger = Button::new("theme-color-trigger")
        .secondary()
        .outline()
        .large()
        .w(px(300.0))
        .h(px(40.0))
        .px(px(12.0))
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .size(px(22.0))
                        .flex_none()
                        .rounded_full()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(color),
                )
                .child(
                    div()
                        .flex_1()
                        .text_left()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(hex.clone()),
                )
                .child(render_icon(AppIcon::ChevronDown, IconTone::Muted, 14.0, cx)),
        );

    Popover::new("theme-color-popover")
        .anchor(Anchor::TopRight)
        .w(px(300.0))
        .p(px(12.0))
        .rounded(cx.theme().radius.min(px(8.0)))
        .border_color(cx.theme().border)
        .bg(cx.theme().tokens.popover)
        .shadow_md()
        .trigger(trigger)
        .child(theme_color_panel(app, color, cx))
        .into_any_element()
}

fn theme_color_panel(app: &OneChat, color: Hsla, cx: &mut Context<OneChat>) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Theme Color"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("Adapts to Light and Dark"),
                        ),
                )
                .child(
                    div()
                        .size(px(32.0))
                        .rounded_full()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(color),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(picker_section_label("Presets", cx))
                .child(
                    div().flex().flex_wrap().gap_2().children(
                        PRESET_COLORS
                            .iter()
                            .map(|value| preset_swatch(value, color, cx)),
                    ),
                ),
        )
        .child(div().h(px(1.0)).bg(cx.theme().border))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(custom_header(app, cx))
                .child(slider_row(
                    "Hue",
                    format!("{:.0}°", color.h * 360.0),
                    hue_track(),
                    &app.settings_ui.theme_color.hue,
                    cx,
                ))
                .child(slider_row(
                    "Saturation",
                    format!("{:.0}%", color.s * 100.0),
                    saturation_track(color),
                    &app.settings_ui.theme_color.saturation,
                    cx,
                ))
                .child(slider_row(
                    "Lightness",
                    format!("{:.0}%", color.l * 100.0),
                    lightness_track(color),
                    &app.settings_ui.theme_color.lightness,
                    cx,
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(cx.theme().muted_foreground)
                        .child("Hex"),
                )
                .child(
                    Input::new(&app.settings_ui.theme_color.hex)
                        .small()
                        .w(px(112.0))
                        .px_2p5(),
                ),
        )
        .into_any_element()
}

fn custom_header(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let at_default = app
        .settings()
        .theme_color
        .eq_ignore_ascii_case(crate::domain::DEFAULT_THEME_COLOR);
    div()
        .h(px(28.0))
        .flex()
        .items_center()
        .justify_between()
        .child(picker_section_label("Custom", cx))
        .child(
            icon_action(
                "reset-theme-color",
                AppIcon::Regenerate,
                IconTone::Muted,
                "Reset to default",
                cx,
            )
            .disabled(at_default)
            .on_click(cx.listener(|this, _, _, cx| this.reset_theme_color(cx))),
        )
        .into_any_element()
}

fn picker_section_label(label: &'static str, cx: &App) -> AnyElement {
    div()
        .text_size(px(11.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(label)
        .into_any_element()
}

fn preset_swatch(value: &'static str, selected: Hsla, cx: &mut Context<OneChat>) -> AnyElement {
    let color = Hsla::parse_hex(value).expect("theme color preset");
    let selected = color.to_hex() == selected.to_hex();
    div()
        .id(SharedString::from(format!("theme-color-{value}")))
        .size(px(36.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_2()
        .border_color(if selected {
            cx.theme().foreground
        } else {
            cx.theme().transparent
        })
        .hover(|style| style.bg(cx.theme().secondary_hover))
        .active(|style| style.bg(cx.theme().secondary_active))
        .child(
            div()
                .size(px(28.0))
                .rounded_full()
                .border_1()
                .border_color(cx.theme().border)
                .bg(color),
        )
        .on_click(cx.listener(move |this, _, _, cx| this.set_theme_color(value.to_string(), cx)))
        .into_any_element()
}

fn slider_row(
    label: &'static str,
    value: String,
    track: AnyElement,
    state: &Entity<SliderState>,
    cx: &App,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_xs()
                .child(label)
                .child(div().text_color(cx.theme().muted_foreground).child(value)),
        )
        .child(
            div()
                .relative()
                .h(px(24.0))
                .flex()
                .items_center()
                .child(track)
                .child(Slider::new(state).w_full().bg(cx.theme().transparent)),
        )
        .into_any_element()
}

fn track(contents: impl IntoElement) -> AnyElement {
    div()
        .absolute()
        .left_0()
        .right_0()
        .h(px(8.0))
        .rounded_full()
        .overflow_hidden()
        .child(contents)
        .into_any_element()
}

fn hue_track() -> AnyElement {
    track(div().size_full().flex().children((0..48).map(|step| {
        div()
            .h_full()
            .flex_1()
            .bg(gpui::hsla(step as f32 / 47.0, 1.0, 0.5, 1.0))
    })))
}

fn saturation_track(color: Hsla) -> AnyElement {
    track(div().size_full().bg(linear_gradient(
        90.0,
        linear_color_stop(gpui::hsla(color.h, 0.0, color.l, 1.0), 0.0),
        linear_color_stop(gpui::hsla(color.h, 1.0, color.l, 1.0), 1.0),
    )))
}

fn lightness_track(color: Hsla) -> AnyElement {
    let middle = gpui::hsla(color.h, color.s, 0.5, 1.0);
    track(
        div()
            .size_full()
            .flex()
            .child(div().h_full().flex_1().bg(linear_gradient(
                90.0,
                linear_color_stop(Hsla::black(), 0.0),
                linear_color_stop(middle, 1.0),
            )))
            .child(div().h_full().flex_1().bg(linear_gradient(
                90.0,
                linear_color_stop(middle, 0.0),
                linear_color_stop(Hsla::white(), 1.0),
            ))),
    )
}
