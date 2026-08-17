mod connection;

use gpui_component::{Sizable as _, select::Select};

use super::*;

pub(super) fn render_tts_top_bar(
    app: &OneChat,
    layout: LayoutClass,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let inspector_open = app.tts.view.inspector_open;
    let has_models = !app.tts.controller.discovery.catalog.tts.is_empty();
    let busy = app.tts.controller.operation.active().is_some();
    let (model_disabled, voice_disabled) =
        tts_selects_disabled(busy, app.tts.controller.discovery.voices.is_empty());
    let controls = if has_models {
        div()
            .min_w_0()
            .flex_none()
            .flex()
            .items_center()
            .gap_2()
            .child(
                Select::new(&app.tts.controls.model)
                    .large()
                    .h(px(36.0))
                    .w(px(if layout.is_wide() { 184.0 } else { 152.0 }))
                    .px_3()
                    .rounded(px(9.0))
                    .placeholder("TTS model")
                    .menu_max_h(px(320.0))
                    .disabled(model_disabled),
            )
            .child(
                Select::new(&app.tts.controls.voice)
                    .large()
                    .h(px(36.0))
                    .w(px(if layout.is_wide() { 156.0 } else { 132.0 }))
                    .px_3()
                    .rounded(px(9.0))
                    .placeholder("Voice")
                    .menu_max_h(px(320.0))
                    .disabled(voice_disabled),
            )
            .child(
                large_icon_button(
                    "toggle-tts-inspector",
                    AppIcon::Sliders,
                    if inspector_open {
                        IconTone::Accent
                    } else {
                        IconTone::Muted
                    },
                    cx,
                )
                .tooltip("TTS tuning")
                .selected(inspector_open)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_tts_inspector_open(!inspector_open, cx)
                })),
            )
            .into_any_element()
    } else {
        div().into_any_element()
    };

    div()
        .h(px(60.0))
        .flex_none()
        .flex()
        .items_center()
        .gap_3()
        .px_4()
        .border_b_1()
        .border_color(cx.theme().border)
        .bg(crate::desktop::ui::theme::palette(cx).toolbar)
        .when(app.settings().sidebar_collapsed, |bar| {
            bar.child(
                large_icon_button("expand-sidebar", AppIcon::Sidebar, IconTone::Muted, cx)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
            )
        })
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap_3()
                .children((!layout.is_narrow()).then(|| {
                    div()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("TTS Playground")
                }))
                .child(connection::render(app, busy, cx)),
        )
        .child(controls)
        .into_any_element()
}

fn tts_selects_disabled(busy: bool, voices_empty: bool) -> (bool, bool) {
    (busy, busy || voices_empty)
}

#[cfg(test)]
mod tests {
    use super::tts_selects_disabled;

    #[test]
    fn busy_tts_operation_disables_model_and_voice_selects() {
        assert_eq!(tts_selects_disabled(true, false), (true, true));
        assert_eq!(tts_selects_disabled(true, true), (true, true));
    }

    #[test]
    fn idle_voice_select_still_requires_discovered_voices() {
        assert_eq!(tts_selects_disabled(false, false), (false, false));
        assert_eq!(tts_selects_disabled(false, true), (false, true));
    }
}
