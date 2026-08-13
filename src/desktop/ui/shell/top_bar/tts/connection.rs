use gpui::{Anchor, BoxShadow, point};
use gpui_component::{
    Disableable as _,
    button::{Button, ButtonVariants as _},
    input::Input,
    popover::Popover,
};

use super::*;
use crate::desktop::ui::icons::IconActionSize::Regular;

pub(super) fn render(app: &OneChat, busy: bool, cx: &mut Context<OneChat>) -> AnyElement {
    let (status, status_color) = connection_status(app, cx);
    let trigger = Button::new("tts-connection-status")
        .ghost()
        .compact()
        .h(px(26.0))
        .px_2()
        .rounded_full()
        .bg(crate::desktop::ui::theme::palette(cx).secondary)
        .tooltip("Configure audio.cpp connection")
        .child(
            div()
                .flex()
                .items_center()
                .gap_1p5()
                .text_size(px(11.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(cx.theme().muted_foreground)
                .child(div().size(px(6.0)).rounded_full().bg(status_color))
                .child(status),
        );
    let palette = *crate::desktop::ui::theme::palette(cx);
    let error = app
        .tts
        .controller
        .discovery
        .error
        .as_ref()
        .map(ToString::to_string);
    let app_entity = cx.entity();

    let panel = div()
        .w(px(380.0))
        .p_4()
        .rounded(px(18.0))
        .border_1()
        .border_color(palette.floating_border)
        .bg(palette.floating_glass)
        .shadow(vec![BoxShadow {
            color: palette.floating_shadow,
            offset: point(px(0.0), px(8.0)),
            blur_radius: px(24.0),
            spread_radius: px(-8.0),
            inset: false,
        }])
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .items_start()
                .gap_3()
                .child(
                    div()
                        .size(px(34.0))
                        .flex_none()
                        .rounded(px(11.0))
                        .bg(palette.accent_soft)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(render_icon(AppIcon::Plug, IconTone::Accent, 17.0, cx)),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("audio.cpp Connection"),
                        )
                        .child(
                            div()
                                .pt_1()
                                .text_size(px(11.0))
                                .line_height(px(16.0))
                                .text_color(cx.theme().muted_foreground)
                                .child("Stored only in memory and discarded when OneChat quits."),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(field_label("Server URL", cx))
                .child(
                    Input::new(&app.tts.controls.connection.endpoint)
                        .aria_label("audio.cpp server URL")
                        .disabled(busy),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(field_label("Bearer Token · Optional", cx))
                .child(
                    Input::new(&app.tts.controls.connection.token)
                        .aria_label("Optional bearer token")
                        .disabled(busy),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .items_start()
                        .gap_2()
                        .text_size(px(10.0))
                        .line_height(px(14.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(render_icon(AppIcon::Info, IconTone::Muted, 13.0, cx))
                        .child(
                            div().min_w_0().flex_1().whitespace_normal().child(
                                "Only for an authenticating proxy. The token is never saved.",
                            ),
                        ),
                ),
        )
        .children(error.map(|error| {
            div()
                .rounded(px(10.0))
                .bg(palette.danger_soft)
                .px_3()
                .py_2()
                .text_size(px(11.0))
                .line_height(px(16.0))
                .text_color(palette.danger)
                .whitespace_normal()
                .child(error)
        }))
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .child(
                    Regular
                        .icon_action(
                            "test-tts-connection",
                            AppIcon::Plug,
                            IconTone::Foreground,
                            "Test connection",
                            cx,
                        )
                        .secondary()
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| this.test_tts_connection(cx))),
                )
                .child(
                    Regular
                        .primary_icon_action(
                            "refresh-tts-models",
                            AppIcon::Regenerate,
                            "Refresh models",
                            cx,
                        )
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_tts_discovery(cx))),
                ),
        );

    Popover::new("tts-connection-popover")
        .anchor(Anchor::TopLeft)
        .appearance(false)
        .open(app.tts.view.connection_popover_open)
        .on_open_change(move |open, _, cx| {
            app_entity.update(cx, |app, cx| {
                app.set_tts_connection_popover_open(*open, cx);
            });
        })
        .trigger(trigger)
        .child(panel)
        .into_any_element()
}

fn connection_status(app: &OneChat, cx: &App) -> (&'static str, gpui::Hsla) {
    if app.tts.controller.discovery.loading {
        ("Connecting…", cx.theme().primary)
    } else if app.tts.controller.discovery.error.is_some() {
        ("Connection failed", cx.theme().danger)
    } else if app
        .tts
        .controller
        .discovery
        .health
        .as_ref()
        .is_some_and(|health| health.ready)
    {
        ("Connected", cx.theme().success)
    } else {
        ("Not connected", cx.theme().muted_foreground)
    }
}

fn field_label(label: &'static str, cx: &App) -> AnyElement {
    div()
        .text_size(px(11.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(label)
        .into_any_element()
}
