mod composer;
mod message;
mod system_prompt;
mod timeline;

use composer::render_composer;
use message::{render_assistant_turn, render_user_turn};
use system_prompt::render_system_prompt_card;
use timeline::TimelineEntry;

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, BoxShadow, Context, ElementId, FontWeight,
    MouseButton, ObjectFit, Role, SharedString, StyledImage as _, div, ease_out_quint, img, point,
    prelude::*, px, rgba,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Selectable as _,
    button::{Button, ButtonVariants as _},
    input::{Enter, Escape as InputEscape, Input, Paste},
};

use super::{
    copy_button::CopyButton,
    icons::{AppIcon, IconTone, render_icon},
    markdown,
    motion::translated_y,
    selectable_text::{SelectableText, selection_color},
    typography::MessageTypography,
};
use crate::{
    desktop::app::{COLLAPSED_THINKING_HEIGHT, OneChat, SystemPromptMode},
    domain::{
        AssistantResponse, MAX_MESSAGE_WIDTH_RATIO, MIN_MESSAGE_WIDTH_RATIO, MessageStatus,
        RequestInfo, SendMessageShortcut, ToolExecution, ToolExecutionStatus, Turn,
    },
};

const MESSAGE_MIN_WIDTH: f32 = 780.0;
const MESSAGE_LIST_HORIZONTAL_PADDING: f32 = 48.0;

fn response_tab_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    typography: MessageTypography,
) -> Button {
    Button::new(id)
        .label(label)
        .h(px(typography.metadata_line_height + 6.0))
        .px_2()
        .rounded(px(8.0))
        .ghost()
        .text_size(px(typography.metadata_size))
        .line_height(px(typography.metadata_line_height))
        .font_weight(FontWeight::SEMIBOLD)
}

fn icon_tooltip(icon: AppIcon) -> &'static str {
    match icon {
        AppIcon::ArrowUp => "Send",
        AppIcon::At => "Add response",
        AppIcon::ChevronDown => "Expand",
        AppIcon::ChevronLeft => "Previous",
        AppIcon::ChevronRight => "Next",
        AppIcon::ChevronUp => "Collapse",
        AppIcon::Close => "Cancel",
        AppIcon::ContextSelect => "Use for context",
        AppIcon::Copy => "Copy",
        AppIcon::FileText => "File",
        AppIcon::Fork => "Fork conversation",
        AppIcon::Info => "Inspect request",
        AppIcon::Layers => "Choose model",
        AppIcon::Pencil => "Edit",
        AppIcon::Regenerate => "Regenerate",
        AppIcon::Save => "Save",
        AppIcon::Stop => "Stop generating",
        _ => "Action",
    }
}

fn icon_button(id: impl Into<ElementId>, icon: AppIcon, tone: IconTone, cx: &App) -> Button {
    Button::new(id)
        .ghost()
        .tooltip(icon_tooltip(icon))
        .size(px(28.0))
        .p_0()
        .child(render_icon(icon, tone, 17.0, cx))
}

fn large_icon_button(id: impl Into<ElementId>, icon: AppIcon, tone: IconTone, cx: &App) -> Button {
    Button::new(id)
        .ghost()
        .tooltip(icon_tooltip(icon))
        .size(px(36.0))
        .p_0()
        .child(render_icon(icon, tone, 20.0, cx))
}

fn primary_icon_button(id: impl Into<ElementId>, icon: AppIcon, cx: &App) -> Button {
    Button::new(id)
        .primary()
        .rounded(px(18.0))
        .tooltip(icon_tooltip(icon))
        .size(px(36.0))
        .p_0()
        .child(render_icon(icon, IconTone::OnAccent, 20.0, cx))
}

pub(crate) fn render(
    app: &OneChat,
    available_width: f32,
    scale_factor: f32,
    jump_to_latest_progress: f32,
    timeline_expansion: f32,
    timeline_focused: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let conversation = app
        .current_conversation()
        .expect("conversation page requires a current conversation");
    let message_max_width =
        message_max_width(available_width, app.settings().message_width_ratio());
    let typography = MessageTypography::new(app.settings().message_font_size());
    let has_system_prompt = !conversation.system_prompt.trim().is_empty();
    let editing_system_prompt = app.chat.system_prompt_mode == SystemPromptMode::Editing;
    let text_selection = app.chat.text_selection.clone();
    text_selection.begin_frame();
    let selection_focus = text_selection.focus_handle().clone();
    let selection_mouse_down = text_selection.clone();
    let selection_mouse_move = text_selection.clone();
    let selection_mouse_up = text_selection.clone();
    let selection_mouse_up_out = text_selection.clone();
    let selection_copy = text_selection.clone();
    let mut messages = div()
        .id("message-list")
        .min_h_0()
        .flex_1()
        .overflow_y_scroll()
        .track_scroll(&app.chat.message_scroll)
        .track_focus(&selection_focus)
        .on_scroll_wheel(cx.listener(OneChat::on_message_scroll))
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            selection_mouse_down.mouse_down(event, window, cx)
        })
        .on_mouse_move(move |event, window, _| selection_mouse_move.mouse_move(event, window))
        .on_mouse_up(MouseButton::Left, move |event, window, _| {
            selection_mouse_up.mouse_up(event, window)
        })
        .on_mouse_up_out(MouseButton::Left, move |event, window, _| {
            selection_mouse_up_out.mouse_up(event, window)
        })
        .on_key_down(move |event, window, cx| selection_copy.copy(event, window, cx))
        .px_6()
        .pt_7()
        .pb_6()
        .children(
            (has_system_prompt || editing_system_prompt)
                .then(|| render_system_prompt_card(app, message_max_width, typography, cx)),
        );

    let mut timeline_entries = Vec::new();
    let mut child_index = usize::from(has_system_prompt || editing_system_prompt);
    if app.current_turns().is_empty() {
        messages = messages.child(render_empty_conversation(app, cx));
    } else {
        for turn in app.current_turns() {
            messages = messages.child(render_user_turn(
                app,
                turn,
                message_max_width,
                typography,
                cx,
            ));
            timeline_entries.push(TimelineEntry {
                item: child_index,
                label: "You".into(),
                timestamp: turn.user.created_at,
            });
            child_index += 1;

            if let Some(response) = app.visible_response(turn) {
                messages = messages.child(render_assistant_turn(
                    app,
                    turn,
                    response,
                    message_max_width,
                    scale_factor,
                    typography,
                    cx,
                ));
                timeline_entries.push(TimelineEntry {
                    item: child_index,
                    label: response.model_name.clone(),
                    timestamp: response.created_at,
                });
                child_index += 1;
            }
        }
    }

    let jump_to_latest_visible =
        !app.chat.follow_latest && !app.chat.message_scroll_motion.is_active();
    let jump_to_latest_offset = if cx.reduce_motion() {
        0.0
    } else {
        8.0 * (1.0 - jump_to_latest_progress)
    };
    let timeline = timeline::render(
        app,
        timeline_entries,
        timeline_expansion,
        timeline_focused,
        cx.reduce_motion(),
        cx,
    );
    let message_area = div()
        .relative()
        .min_h_0()
        .flex_1()
        .flex()
        .child(messages)
        .children(timeline)
        .children((jump_to_latest_progress > 0.0).then(|| {
            let dark = cx.theme().is_dark();
            let glass = if dark {
                rgba(0x2c2c2ef2)
            } else {
                rgba(0xfffffff2)
            };
            let glass_border = if dark {
                rgba(0xffffff38)
            } else {
                rgba(0x3c3c4326)
            };
            let glass_shadow = vec![BoxShadow {
                color: if dark {
                    rgba(0x0000005c).into()
                } else {
                    rgba(0x1d1d1f24).into()
                },
                offset: point(px(0.0), px(6.0)),
                blur_radius: px(18.0),
                spread_radius: px(-7.0),
                inset: false,
            }];
            translated_y(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom(px(12.0))
                    .flex()
                    .justify_center()
                    .opacity(jump_to_latest_progress)
                    .child(
                        div()
                            .id("jump-to-latest")
                            .rounded(px(20.0))
                            .h(px(40.0))
                            .px_3()
                            .bg(glass)
                            .border_1()
                            .border_color(glass_border)
                            .shadow(glass_shadow)
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .cursor_default()
                            .child(render_icon(
                                AppIcon::ArrowDown,
                                IconTone::Foreground,
                                16.0,
                                cx,
                            ))
                            .child("Jump to Latest")
                            .when(jump_to_latest_visible, |button| {
                                button
                                    .role(Role::Button)
                                    .aria_label("Jump to Latest")
                                    .on_click(cx.listener(|this, _, _, cx| this.jump_to_latest(cx)))
                            }),
                    ),
                px(jump_to_latest_offset),
            )
        }));

    div()
        .size_full()
        .flex()
        .flex_col()
        .child(message_area)
        .child(render_composer(app, message_max_width, typography, cx))
        .into_any_element()
}

fn message_max_width(available_width: f32, ratio: f32) -> f32 {
    let content_width = (available_width - MESSAGE_LIST_HORIZONTAL_PADDING).max(0.0);
    let ratio = ratio.clamp(MIN_MESSAGE_WIDTH_RATIO, MAX_MESSAGE_WIDTH_RATIO);
    (available_width * ratio)
        .max(MESSAGE_MIN_WIDTH)
        .min(content_width)
}

fn render_empty_conversation(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    div()
        .mx_auto()
        .w_full()
        .max_w(px(780.0))
        .min_h(px(300.0))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .max_w(px(520.0))
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .text_center()
                .child(
                    div()
                        .size(px(48.0))
                        .rounded_full()
                        .bg(cx.theme().accent)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(cx.theme().primary)
                        .text_lg()
                        .child(render_icon(AppIcon::Sparkles, IconTone::Accent, 22.0, cx)),
                )
                .child(
                    div()
                        .pt_2()
                        .text_size(px(26.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("What can I help with?"),
                )
                .child(
                    div()
                        .line_height(px(22.0))
                        .text_color(cx.theme().muted_foreground)
                        .child("Ask a question, explore an idea, or make something new."),
                )
                .children(app.current_model().is_none().then(|| {
                    primary_icon_button("empty-choose-model", AppIcon::Layers, cx).on_click(
                        cx.listener(|this, _, window, cx| this.open_model_picker(window, cx)),
                    )
                })),
        )
        .into_any_element()
}
