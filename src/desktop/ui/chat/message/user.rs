use super::*;

mod actions;
mod content;
mod edit_attachments;

use actions::render_message_actions;
use content::render_message_content;

pub(in crate::desktop::ui::chat) fn render_user_turn(
    app: &OneChat,
    turn: &Turn,
    message_max_width: f32,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    animated_message(
        render_user_message(app, turn, message_max_width, scale_factor, typography, cx),
        format!("user-{}", turn.id),
    )
}

fn render_user_message(
    app: &OneChat,
    turn: &Turn,
    message_max_width: f32,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let user_message_max_width = message_max_width * USER_MESSAGE_WIDTH_RATIO;
    let action_group: SharedString = format!("user-actions-{}", turn.id).into();
    let content = render_message_content(
        app,
        turn,
        user_message_max_width,
        scale_factor,
        typography,
        cx,
    );
    let action_bar = render_message_actions(app, turn, action_group.clone(), typography, cx);
    div()
        .mx_auto()
        .mb_7()
        .w_full()
        .max_w(px(message_max_width))
        .flex()
        .justify_end()
        .child(
            div()
                .group(action_group)
                .max_w(px(user_message_max_width))
                .min_w_0()
                .flex()
                .flex_col()
                .items_end()
                .child(content)
                .child(action_bar),
        )
        .into_any_element()
}
