use super::*;

pub(super) fn render_composer(
    app: &OneChat,
    has_system_prompt: bool,
    editing_system_prompt: bool,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let generating = app.is_current_generating();
    let can_send = !app.chat.composer.read(cx).text().trim().is_empty()
        && app.current_model().is_some()
        && app.current_conversation().is_some();
    let action = if generating {
        destructive_icon_button("composer-stop", "■", colors)
            .on_click(cx.listener(|this, _, _, cx| this.stop_current_generation(cx)))
    } else if can_send {
        primary_icon_button("composer-send", "↑", colors)
            .on_click(cx.listener(|this, _, _, cx| this.send_composer(cx)))
    } else {
        primary_icon_button("composer-send-disabled", "↑", colors)
            .opacity(0.38)
            .cursor_default()
    };

    let (previous_lines, visual_lines, height_revision) =
        app.chat.composer.read(cx).height_transition();
    let previous_height = 50.0 + (previous_lines.saturating_sub(1) as f32 * 24.0);
    let target_height = 50.0 + (visual_lines.saturating_sub(1) as f32 * 24.0);
    let input = div()
        .min_w_0()
        .flex_1()
        .overflow_hidden()
        .child(app.chat.composer.clone())
        .with_animation(
            SharedString::from(format!("composer-height-{height_revision}")),
            Animation::new(Duration::from_millis(200)).with_easing(ease_out_quint()),
            move |input, delta| {
                input.opacity(0.86 + delta * 0.14).max_h(px(
                    previous_height + (target_height - previous_height) * delta
                ))
            },
        );

    div()
        .flex_none()
        .w_full()
        .px_6()
        .pb_5()
        .child(
            div()
                .mx_auto()
                .w_full()
                .max_w(px(800.0))
                .child(
                    div()
                        .pb_2()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap_1()
                                .children((!has_system_prompt && !editing_system_prompt).then(
                                    || {
                                        compact_button(
                                            "composer-add-system-prompt",
                                            "+ System Prompt",
                                            colors,
                                        )
                                        .text_color(colors.accent)
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.begin_edit_system_prompt(cx)
                                            }),
                                        )
                                    },
                                ))
                                .children((has_system_prompt || editing_system_prompt).then(|| {
                                    compact_button("composer-system", "System Prompt", colors)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.begin_edit_system_prompt(cx)
                                        }))
                                }))
                                .child(
                                    compact_button("composer-context", "Context", colors).on_click(
                                        cx.listener(|this, _, _, cx| {
                                            this.open_inspector(InspectorTab::Context, cx)
                                        }),
                                    ),
                                )
                                .child(
                                    compact_button("composer-parameters", "Parameters", colors)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.open_inspector(InspectorTab::Model, cx)
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(colors.muted)
                                .child("↩ Send  ·  ⇧↩ New Line"),
                        ),
                )
                .child(div().flex().items_end().gap_2().child(input).child(action)),
        )
        .into_any_element()
}
