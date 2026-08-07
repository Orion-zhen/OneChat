use super::*;

pub(super) fn render_destructive_confirmation(
    action: &DestructiveAction,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let (title, detail, confirm_label) = match action {
        DestructiveAction::DeleteConversation { title, .. } => (
            "Delete Conversation?",
            format!("“{title}” and all of its messages will be removed from this Mac."),
            "Delete",
        ),
        DestructiveAction::DeleteProvider { name, .. } => (
            "Delete Provider?",
            format!("“{name}” and its configured models will be removed from this Mac."),
            "Delete Provider",
        ),
        DestructiveAction::DeleteModel { name, .. } => (
            "Delete Model?",
            format!("“{name}” will no longer be available to conversations."),
            "Delete Model",
        ),
        DestructiveAction::ClearContext { .. } => (
            "Clear Conversation?",
            "All messages and request details in this conversation will be permanently removed."
                .to_string(),
            "Clear",
        ),
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
                .text_xl()
                .text_color(colors.danger)
                .child("!"),
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
                .max_w(px(350.0))
                .text_sm()
                .line_height(px(21.0))
                .text_color(colors.muted)
                .child(detail),
        )
        .child(
            div()
                .w_full()
                .pt_2()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    svg_icon_button(
                        "cancel-destructive-action",
                        UiIcon::Close,
                        IconTone::Muted,
                        colors,
                        scale_factor,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_destructive_action(cx))),
                )
                .child(
                    destructive_button("confirm-destructive-action", confirm_label, colors)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.confirm_destructive_action(cx)),
                        ),
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
