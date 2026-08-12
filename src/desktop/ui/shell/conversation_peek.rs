use super::*;
use crate::{
    desktop::{app::ConversationPeekContent, ui::text::summary as text_summary},
    domain::{Turn, active_turns},
};

const PEEK_WIDTH: f32 = 420.0;
const PEEK_ESTIMATED_HEIGHT: f32 = 444.0;
const PEEK_MARGIN: f32 = 12.0;
const PEEK_TOP_INSET: f32 = 68.0;
const PEEK_TURN_LIMIT: usize = 3;

pub(super) fn render_conversation_peek(
    app: &OneChat,
    sidebar_width: f32,
    window_height: f32,
    cx: &App,
) -> Option<AnyElement> {
    let peek = &app.sidebar.conversation_peek;
    let conversation_id = peek.conversation_id.as_deref()?;
    let conversation = app
        .data
        .snapshot
        .conversations
        .iter()
        .find(|conversation| conversation.id == conversation_id)?;
    let model = conversation
        .model_id
        .as_deref()
        .and_then(|model_id| {
            app.data
                .snapshot
                .models
                .iter()
                .find(|model| model.id == model_id)
        })
        .map_or("Default model", |model| model.display_name.as_str());
    let max_top = (window_height - PEEK_ESTIMATED_HEIGHT - PEEK_MARGIN).max(PEEK_TOP_INSET);
    let top = (peek.anchor_y - 32.0).clamp(PEEK_TOP_INSET, max_top);
    let palette = *crate::desktop::ui::theme::palette(cx);

    let body = match &peek.content {
        ConversationPeekContent::Loading => peek_notice("Loading conversation…", cx),
        ConversationPeekContent::Failed => {
            peek_notice("This conversation could not be previewed.", cx)
        }
        ConversationPeekContent::Ready(turns) => render_turns(turns, cx),
    };

    Some(
        div()
            .id("conversation-peek")
            .absolute()
            .occlude()
            .left(px(sidebar_width + PEEK_MARGIN))
            .top(px(top))
            .w(px(PEEK_WIDTH))
            .max_h(px(PEEK_ESTIMATED_HEIGHT))
            .overflow_hidden()
            .rounded(px(18.0))
            .border_1()
            .border_color(palette.floating_border)
            .bg(palette.floating_glass)
            .shadow_xl()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_4()
                    .pt_4()
                    .pb_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .truncate()
                            .text_size(px(15.0))
                            .line_height(px(20.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(conversation.title.clone()),
                    )
                    .child(
                        div()
                            .pt_1()
                            .truncate()
                            .text_size(px(11.0))
                            .line_height(px(15.0))
                            .text_color(cx.theme().muted_foreground)
                            .child(model.to_string()),
                    ),
            )
            .child(
                div()
                    .min_h_0()
                    .flex_1()
                    .overflow_y_hidden()
                    .p_4()
                    .child(body),
            )
            .child(
                div()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_size(px(10.0))
                    .line_height(px(14.0))
                    .text_color(cx.theme().muted_foreground)
                    .child("Release to close"),
            )
            .into_any_element(),
    )
}

fn render_turns(turns: &[Turn], cx: &App) -> AnyElement {
    let active = active_turns(turns);
    let start = active.len().saturating_sub(PEEK_TURN_LIMIT);
    if active.is_empty() {
        return peek_notice("No messages yet.", cx);
    }

    div()
        .flex()
        .flex_col()
        .gap_3()
        .children(active[start..].iter().map(|turn| render_turn(turn, cx)))
        .into_any_element()
}

fn render_turn(turn: &Turn, cx: &App) -> AnyElement {
    let attachment_count = turn.user.attachments.len();
    let user_summary = text_summary(
        &turn.user.content,
        120,
        Some(if attachment_count == 0 {
            "No message text"
        } else {
            "Attachment"
        }),
    );
    let user_metadata = match attachment_count {
        0 => "You".to_string(),
        1 => "You · 1 attachment".to_string(),
        count => format!("You · {count} attachments"),
    };
    let response = turn.continuation_response();

    div()
        .rounded(px(12.0))
        .bg(cx.theme().muted)
        .p_3()
        .flex()
        .flex_col()
        .gap_2()
        .child(peek_message(user_metadata, user_summary, true, cx))
        .children(response.map(|response| {
            peek_message(
                response.model_name.clone(),
                text_summary(&response.content, 180, Some("No response text")),
                false,
                cx,
            )
        }))
        .into_any_element()
}

fn peek_message(label: String, summary: String, user: bool, cx: &App) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(10.0))
                .line_height(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if user {
                    cx.theme().primary
                } else {
                    cx.theme().muted_foreground
                })
                .child(label),
        )
        .child(
            div()
                .line_clamp(3)
                .text_size(px(12.0))
                .line_height(px(17.0))
                .child(summary),
        )
        .into_any_element()
}

fn peek_notice(message: &'static str, cx: &App) -> AnyElement {
    div()
        .min_h(px(92.0))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .text_color(cx.theme().muted_foreground)
        .child(message)
        .into_any_element()
}
