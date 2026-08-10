use super::*;

pub(super) fn render_top_bar(
    app: &OneChat,
    animated_title: Option<&str>,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    if app.navigation.page == Page::Settings {
        return div()
            .h(px(60.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px_5()
            .bg(cx.theme().title_bar)
            .shadow_xs()
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Settings"),
            )
            .child(
                large_icon_button("chat-page", AppIcon::Close, IconTone::Muted, cx)
                    .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Chat, cx))),
            )
            .into_any_element();
    }

    let current_conversation = app.current_conversation();
    let inspector_open = app.navigation.inspector_open;
    let title = animated_title.map(str::to_string).unwrap_or_else(|| {
        current_conversation
            .map(|conversation| conversation.title.clone())
            .unwrap_or_else(|| "OneChat".into())
    });
    let title_waiting = current_conversation
        .is_some_and(|conversation| conversation.auto_title_state == AutoTitleState::Running);
    let title_animation_id: SharedString = current_conversation.map_or_else(
        || "waiting-top-bar-title".into(),
        |conversation| format!("waiting-top-bar-title-{}", conversation.id).into(),
    );
    let selected_model = app.selected_model();
    let provider = selected_model.and_then(|model| app.provider_for_model(model));
    let provider_name = provider
        .map(|provider| provider.name.clone())
        .unwrap_or_else(|| "No provider".into());
    let model_label = selected_model
        .map(|model| model.display_name.clone())
        .unwrap_or_else(|| "Choose Model".into());
    let model_icon = selected_model.map_or(AppIcon::Bot, |model| {
        if model.capabilities.vision {
            AppIcon::Eye
        } else {
            AppIcon::MessageText
        }
    });
    let prompt_label = current_conversation
        .map(|conversation| app.system_prompt_label(&conversation.system_prompt))
        .unwrap_or_else(|| "None".into());
    let reasoning_label = current_conversation.and_then(|conversation| {
        let reasoning = selected_model?.reasoning.as_ref()?;
        let selected = app
            .chat
            .generation_config_editor
            .as_ref()
            .and_then(|editor| editor.reasoning_preset())
            .or(conversation.generation_config.reasoning_preset.as_deref())
            .unwrap_or_else(|| reasoning.default_preset());
        reasoning
            .preset_options()
            .into_iter()
            .find(|(id, _)| id == selected)
            .map(|(_, label)| label)
    });
    let can_choose_reasoning = current_conversation.is_some() && !app.is_current_generating();
    let can_choose_prompt = current_conversation.is_some() && !app.is_current_generating();
    let tool_label = current_conversation.map_or_else(
        || "Tools".to_string(),
        |conversation| {
            let selected_count = app
                .mcp
                .snapshot
                .servers
                .iter()
                .filter(|server| server.enabled && server.status == McpServerStatus::Ready)
                .flat_map(|server| {
                    server.tools.iter().filter(move |tool| {
                        conversation
                            .tool_selection
                            .resolves(&server.id, &tool.name, tool.enabled)
                    })
                })
                .count();
            format!("{selected_count} Tools")
        },
    );
    let (connection, connection_color) = provider.map_or(
        ("Not configured", cx.theme().muted_foreground),
        |provider| match app.settings_ui.connection_tests.get(&provider.id) {
            Some(ConnectionTestStatus::Testing) => ("Testing", cx.theme().primary),
            Some(ConnectionTestStatus::Connected) => ("Connected", cx.theme().success),
            Some(ConnectionTestStatus::Failed(_)) => ("Connection failed", cx.theme().danger),
            None if provider.enabled => ("Ready", cx.theme().success),
            None => ("Disabled", cx.theme().danger),
        },
    );

    div()
        .h(px(60.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .px_4()
        .bg(cx.theme().title_bar)
        .shadow_xs()
        .when(app.settings().sidebar_collapsed, |this| {
            this.child(
                large_icon_button("expand-sidebar", AppIcon::Sidebar, IconTone::Muted, cx)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
            )
        })
        .child(
            div()
                .min_w_0()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .justify_center()
                .child(waiting_title(
                    div()
                        .max_w(px(400.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                    title_animation_id,
                    title_waiting,
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(div().size(px(6.0)).rounded_full().bg(connection_color))
                        .child(format!("{provider_name} · {connection}")),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    button_base("open-model-picker")
                        .large()
                        .h(px(40.0))
                        .px(px(14.0))
                        .rounded(px(12.0))
                        .tooltip("Choose model")
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(render_icon(model_icon, IconTone::Muted, 14.0, cx))
                        .child(div().whitespace_nowrap().child(model_label))
                        .child(render_icon(AppIcon::ChevronDown, IconTone::Muted, 14.0, cx))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_model_picker(window, cx)),
                        ),
                )
                .children(reasoning_label.map(|reasoning_label| {
                    button_base("open-reasoning-picker")
                        .large()
                        .h(px(40.0))
                        .px(px(14.0))
                        .rounded(px(12.0))
                        .tooltip("Choose reasoning preset")
                        .disabled(!can_choose_reasoning)
                        .max_w(px(190.0))
                        .flex()
                        .items_center()
                        .gap_2()
                        .on_click(
                            cx.listener(|this, _, window, cx| {
                                this.open_reasoning_picker(window, cx)
                            }),
                        )
                        .child(render_icon(AppIcon::Brain, IconTone::Muted, 14.0, cx))
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(reasoning_label),
                        )
                        .child(render_icon(AppIcon::ChevronDown, IconTone::Muted, 14.0, cx))
                }))
                .child(
                    button_base("open-prompt-picker")
                        .large()
                        .h(px(40.0))
                        .px(px(14.0))
                        .rounded(px(12.0))
                        .tooltip("Choose system prompt")
                        .disabled(!can_choose_prompt)
                        .max_w(px(190.0))
                        .flex()
                        .items_center()
                        .gap_2()
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_prompt_picker(window, cx)),
                        )
                        .child(render_icon(AppIcon::Command, IconTone::Muted, 14.0, cx))
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(prompt_label),
                        )
                        .child(render_icon(AppIcon::ChevronDown, IconTone::Muted, 14.0, cx)),
                )
                .child(
                    button_base("open-tools-inspector")
                        .large()
                        .h(px(40.0))
                        .px(px(12.0))
                        .rounded(px(12.0))
                        .tooltip("Configure tools for this conversation")
                        .disabled(current_conversation.is_none())
                        .max_w(px(170.0))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(render_icon(AppIcon::Plug, IconTone::Muted, 14.0, cx))
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(tool_label),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.open_tools_inspector(cx))),
                )
                .child(
                    large_icon_button(
                        "toggle-inspector",
                        AppIcon::Info,
                        if inspector_open {
                            IconTone::Accent
                        } else {
                            IconTone::Muted
                        },
                        cx,
                    )
                    .selected(inspector_open)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_inspector_visible(!inspector_open, cx)
                    })),
                ),
        )
        .into_any_element()
}
