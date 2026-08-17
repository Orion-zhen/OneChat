use super::*;

pub(super) fn render_chat_top_bar(
    app: &OneChat,
    animated_title: Option<&str>,
    layout: LayoutClass,
    cx: &mut Context<OneChat>,
) -> AnyElement {
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
    let model_capabilities = selected_model.map(|model| &model.capabilities);
    let prompt_label = current_conversation
        .map(|conversation| app.prompt_setup_label(conversation))
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
                .children((!layout.is_narrow()).then(|| {
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(div().size(px(6.0)).rounded_full().bg(connection_color))
                        .child(format!("{provider_name} · {connection}"))
                })),
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
                        .child(render_model_capability_icon(model_capabilities, cx))
                        .children((!layout.is_narrow()).then(|| {
                            div()
                                .max_w(px(180.0))
                                .min_w_0()
                                .truncate()
                                .child(model_label)
                        }))
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
                }))
                .child(
                    button_base("open-prompt-picker")
                        .large()
                        .h(px(40.0))
                        .px(px(14.0))
                        .rounded(px(12.0))
                        .tooltip("Choose prompt preset")
                        .disabled(!can_choose_prompt)
                        .max_w(px(190.0))
                        .flex()
                        .items_center()
                        .gap_2()
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_prompt_picker(window, cx)),
                        )
                        .child(render_icon(AppIcon::Command, IconTone::Muted, 14.0, cx))
                        .children(layout.is_wide().then(|| {
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(prompt_label)
                        }))
                        .children(
                            layout.is_wide().then(|| {
                                render_icon(AppIcon::ChevronDown, IconTone::Muted, 14.0, cx)
                            }),
                        ),
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
                        .children(layout.is_wide().then(|| {
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(tool_label)
                        }))
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
