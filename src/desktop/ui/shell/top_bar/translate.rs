use super::*;

pub(super) fn render_translation_top_bar(
    app: &OneChat,
    layout: LayoutClass,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let generating = app.translation.is_generating();
    let model = app.translation_model();
    let model_label = model
        .map(|model| model.display_name.clone())
        .unwrap_or_else(|| "Choose Model".into());
    let model_capabilities = model.map(|model| &model.capabilities);
    let reasoning_label = model.and_then(|model| {
        let reasoning = model.reasoning.as_ref()?;
        let selected = app
            .translation
            .reasoning_preset
            .as_deref()
            .unwrap_or_else(|| reasoning.default_preset());
        reasoning
            .preset_options()
            .into_iter()
            .find(|(id, _)| id == selected)
            .map(|(_, label)| label)
            .or_else(|| {
                reasoning
                    .preset_options()
                    .into_iter()
                    .find(|(id, _)| id == reasoning.default_preset())
                    .map(|(_, label)| label)
            })
    });

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
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Translation Playground")
                }))
                .child(
                    button_base("swap-translation-languages")
                        .ghost()
                        .size(px(36.0))
                        .p_0()
                        .rounded(px(18.0))
                        .tooltip("Swap languages")
                        .disabled(generating)
                        .child(render_icon(
                            AppIcon::ArrowLeftRight,
                            IconTone::Muted,
                            17.0,
                            cx,
                        ))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.swap_translation_languages(window, cx)
                        })),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    button_base("translation-model-picker")
                        .large()
                        .h(px(40.0))
                        .px(px(14.0))
                        .rounded(px(12.0))
                        .tooltip("Choose translation model")
                        .disabled(generating)
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(render_model_capability_icon(model_capabilities, cx))
                        .children(
                            (!layout.is_narrow())
                                .then(|| div().whitespace_nowrap().child(model_label)),
                        )
                        .children(
                            (!layout.is_narrow()).then(|| {
                                render_icon(AppIcon::ChevronDown, IconTone::Muted, 14.0, cx)
                            }),
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_model_picker(window, cx)),
                        ),
                )
                .children(reasoning_label.map(|reasoning_label| {
                    button_base("translation-reasoning-picker")
                        .large()
                        .h(px(40.0))
                        .px(px(14.0))
                        .rounded(px(12.0))
                        .tooltip("Choose translation reasoning preset")
                        .disabled(generating)
                        .max_w(px(190.0))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(render_icon(AppIcon::Brain, IconTone::Muted, 14.0, cx))
                        .children(layout.is_wide().then(|| {
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(reasoning_label)
                        }))
                        .children(
                            layout.is_wide().then(|| {
                                render_icon(AppIcon::ChevronDown, IconTone::Muted, 14.0, cx)
                            }),
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| {
                                this.open_reasoning_picker(window, cx)
                            }),
                        )
                }))
                .child(
                    Button::new("translation-primary-action")
                        .when(generating, |button| button.danger())
                        .when(!generating, |button| button.primary())
                        .h(px(38.0))
                        .px(px(if layout.is_narrow() { 11.0 } else { 14.0 }))
                        .rounded(px(11.0))
                        .tooltip(if generating {
                            "Stop translation"
                        } else {
                            "Translate (⌘↩)"
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(render_icon(
                                    if generating {
                                        AppIcon::Stop
                                    } else {
                                        AppIcon::Continue
                                    },
                                    IconTone::OnAccent,
                                    16.0,
                                    cx,
                                ))
                                .children((!layout.is_narrow()).then_some(if generating {
                                    "Stop"
                                } else {
                                    "Translate"
                                })),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if generating {
                                this.stop_translation(cx);
                            } else {
                                this.start_translation(cx);
                            }
                        })),
                ),
        )
        .into_any_element()
}

fn render_model_capability_icon(capabilities: Option<&ModelCapabilities>, cx: &App) -> AnyElement {
    let Some(capabilities) = capabilities else {
        return render_icon(AppIcon::Bot, IconTone::Muted, 14.0, cx);
    };

    match (capabilities.vision, capabilities.audio_input) {
        (false, false) => render_icon(AppIcon::MessageText, IconTone::Muted, 14.0, cx),
        (true, false) => render_icon(AppIcon::Eye, IconTone::Muted, 14.0, cx),
        (false, true) => render_icon(AppIcon::AudioLines, IconTone::Muted, 14.0, cx),
        (true, true) => render_icon(AppIcon::Shapes, IconTone::Muted, 14.0, cx),
    }
}
