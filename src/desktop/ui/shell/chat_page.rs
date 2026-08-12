use super::*;

pub(super) fn render_chat_page(
    app: &OneChat,
    available_width: f32,
    scale_factor: f32,
    jump_to_latest_progress: f32,
    timeline_expansion: f32,
    timeline_focused: bool,
    context_usage_popover_progress: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let content = if app.data.loading {
        empty_state(
            "Loading your conversations",
            "Opening the local OneChat library…",
            None,
            cx,
        )
    } else if app.data.snapshot.providers.is_empty() {
        empty_state(
            "Connect a provider",
            "Add OpenAI, Anthropic, Gemini, or an OpenAI-compatible provider to get started.",
            Some(("Open Settings", Page::Settings)),
            cx,
        )
    } else if !app
        .data
        .snapshot
        .models
        .iter()
        .any(|model| app.model_availability(model).is_ok())
    {
        empty_state(
            "Add your first model",
            "Choose a remote model ID for one of your configured providers.",
            Some(("Manage Models", Page::Settings)),
            cx,
        )
    } else if app.data.snapshot.conversations.is_empty() {
        empty_state(
            "What would you like to explore?",
            "Conversations and credentials stay on this Mac.",
            Some(("New Conversation", Page::Chat)),
            cx,
        )
    } else if app.current_conversation().is_none() {
        empty_state(
            "Choose a conversation",
            "Select one from the sidebar or start a new conversation.",
            None,
            cx,
        )
    } else {
        chat::render(
            app,
            available_width,
            scale_factor,
            jump_to_latest_progress,
            timeline_expansion,
            timeline_focused,
            context_usage_popover_progress,
            cx,
        )
    };

    let drop_enabled = !app.is_current_generating()
        && !app.chat.attachments_loading
        && app.current_model().is_some()
        && app.current_conversation().is_some()
        && app.chat.attachments.len() < crate::application::attachments::MAX_ATTACHMENTS;
    let palette = *crate::desktop::ui::theme::palette(cx);
    let drop_overlay = div()
        .absolute()
        .top_2()
        .right_2()
        .bottom_2()
        .left_2()
        .invisible()
        .can_drop(move |value, _, _| {
            drop_enabled && value.downcast_ref::<gpui::ExternalPaths>().is_some()
        })
        .group_drag_over::<gpui::ExternalPaths>(ATTACHMENT_DROP_GROUP, |style| style.visible())
        .rounded(px(18.0))
        .border_2()
        .border_dashed()
        .border_color(palette.accent_border)
        .bg(palette.accent_soft.opacity(0.82))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .rounded(px(24.0))
                .border_1()
                .border_color(palette.floating_border)
                .bg(palette.floating_glass)
                .shadow_lg()
                .px_4()
                .py_3()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .size(px(40.0))
                        .rounded(px(20.0))
                        .bg(palette.accent)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(render_icon(AppIcon::FileUp, IconTone::OnAccent, 20.0, cx)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Add to this message"),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted_foreground)
                                .child("Drop files to attach"),
                        ),
                ),
        )
        .on_drop(cx.listener(|this, paths: &gpui::ExternalPaths, _, cx| {
            this.add_dropped_attachments(paths.paths().to_vec(), cx)
        }));

    div()
        .relative()
        .group(ATTACHMENT_DROP_GROUP)
        .min_w_0()
        .flex_1()
        .h_full()
        .can_drop(move |value, _, _| {
            drop_enabled && value.downcast_ref::<gpui::ExternalPaths>().is_some()
        })
        .on_drop(cx.listener(|this, paths: &gpui::ExternalPaths, _, cx| {
            this.add_dropped_attachments(paths.paths().to_vec(), cx)
        }))
        .child(content)
        .child(drop_overlay)
        .into_any_element()
}

fn empty_state(
    title: &'static str,
    detail: &'static str,
    action: Option<(&'static str, Page)>,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let action = action.map(|(label, page)| {
        if label == "New Conversation" {
            primary_icon_button("empty-new-conversation", AppIcon::Plus, cx)
                .on_click(cx.listener(|this, _, _, cx| this.create_conversation(cx)))
        } else {
            primary_icon_button(
                "empty-state-action",
                if label == "Manage Models" {
                    AppIcon::Layers
                } else {
                    AppIcon::Settings
                },
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| this.set_page(page, cx)))
        }
    });
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p_8()
        .child(
            div()
                .max_w(px(500.0))
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .text_center()
                .child(
                    div()
                        .size(px(52.0))
                        .rounded_full()
                        .bg(cx.theme().accent)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xl()
                        .text_color(cx.theme().primary)
                        .child("✦"),
                )
                .child(
                    div()
                        .pt_2()
                        .text_size(px(24.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .max_w(px(440.0))
                        .line_height(px(22.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(detail),
                )
                .children(action),
        )
        .into_any_element()
}
