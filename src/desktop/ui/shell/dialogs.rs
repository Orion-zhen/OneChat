use super::*;

pub(super) fn render_destructive_confirmation(
    action: &DestructiveAction,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let title = match action {
        DestructiveAction::DeleteConversation { .. } => "Delete Conversation?",
        DestructiveAction::DeleteProvider { .. } => "Delete Provider?",
        DestructiveAction::DeleteModel { .. } => "Delete Model?",
        DestructiveAction::DeletePromptPreset { .. } => "Delete Prompt Preset?",
        DestructiveAction::ClearContext { .. } => "Clear Conversation?",
    };
    let panel = div()
        .w_full()
        .max_w(px(420.0))
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .shadow_lg()
        .p_5()
        .flex()
        .flex_col()
        .items_center()
        .gap_3()
        .text_center()
        .child(
            div()
                .size(px(48.0))
                .rounded_full()
                .bg(if colors.dark {
                    rgba(0xff453a24)
                } else {
                    rgba(0xd7001518)
                })
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(
                    Icon::AlertTriangle,
                    IconTone::Danger,
                    colors,
                    scale_factor,
                    24.0,
                )),
        )
        .child(
            div()
                .pt_1()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .w_full()
                .pt_2()
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .child(
                    large_icon_button(
                        "cancel-destructive-action",
                        Icon::Close,
                        IconTone::Muted,
                        colors,
                        scale_factor,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_destructive_action(cx))),
                )
                .child(
                    large_icon_button(
                        "confirm-destructive-action",
                        Icon::Trash,
                        IconTone::Danger,
                        colors,
                        scale_factor,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.confirm_destructive_action(cx))),
                ),
        );
    animated_overlay(
        panel,
        colors,
        "destructive-confirmation-backdrop",
        "destructive-confirmation-panel",
    )
}

pub(super) fn animated_overlay(
    panel: Div,
    colors: Colors,
    backdrop_id: &'static str,
    panel_id: &'static str,
) -> AnyElement {
    let duration = 220;
    let panel = panel
        .with_animation(
            panel_id,
            Animation::new(Duration::from_millis(duration)).with_easing(ease_out_quint()),
            |panel, delta| {
                panel
                    .opacity(0.68 + delta * 0.32)
                    .mt(px(14.0 * (1.0 - delta)))
            },
        )
        .into_any_element();

    div()
        .id(backdrop_id)
        .occlude()
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .left_0()
        .flex()
        .items_start()
        .justify_center()
        .pt(px(96.0))
        .px_5()
        .bg(colors.scrim)
        .child(panel)
        .with_animation(
            backdrop_id,
            Animation::new(Duration::from_millis(duration)).with_easing(ease_out_quint()),
            |backdrop, delta| backdrop.opacity(delta),
        )
        .into_any_element()
}
