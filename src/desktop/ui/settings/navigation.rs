use super::*;

#[derive(Clone)]
struct ProviderDrag {
    id: String,
    name: SharedString,
}

impl Render for ProviderDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .rounded(px(10.0))
            .bg(cx.theme().accent)
            .text_color(cx.theme().accent_foreground)
            .text_sm()
            .shadow_md()
            .child(self.name.clone())
    }
}

pub(super) fn settings_sidebar(app: &OneChat, width: f32, cx: &mut Context<OneChat>) -> AnyElement {
    let general_selected = app.settings_ui.section == SettingsSection::General;
    let default_models_selected = app.settings_ui.section == SettingsSection::DefaultModels;
    let prompts_selected = app.settings_ui.section == SettingsSection::SystemPrompts;
    let mcp_selected = app.settings_ui.section == SettingsSection::Mcp;
    let mut providers = div().flex().flex_col().gap_1().py(px(3.0));

    for (index, provider) in app.data.snapshot.providers.iter().enumerate() {
        let selected = app.settings_ui.section == SettingsSection::Provider(provider.id.clone());
        let model_count = app
            .data
            .snapshot
            .models
            .iter()
            .filter(|model| model.provider_id == provider.id)
            .count();
        let drop_after = app
            .settings_ui
            .provider_drop_target
            .as_ref()
            .filter(|(id, _)| id == &provider.id)
            .map(|(_, after)| *after);
        providers = providers.child(provider_nav_row(
            provider,
            index,
            model_count,
            selected,
            drop_after,
            cx,
        ));
    }

    if app.data.snapshot.providers.is_empty() {
        providers = providers.child(
            div()
                .px_3()
                .py_2()
                .text_size(px(12.0))
                .text_color(cx.theme().muted_foreground)
                .child("No providers configured"),
        );
    }

    div()
        .w(px(width))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().sidebar)
        .child(
            div()
                .px_3()
                .pt_4()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    settings_nav_row(
                        "settings-general",
                        AppIcon::Sliders,
                        "General",
                        "Appearance and behavior",
                        general_selected,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.request_select_settings_section(SettingsSection::General, window, cx)
                    })),
                )
                .child(
                    settings_nav_row(
                        "settings-default-models",
                        AppIcon::Layers,
                        "Default Models",
                        "Models for new conversations",
                        default_models_selected,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.request_select_settings_section(
                            SettingsSection::DefaultModels,
                            window,
                            cx,
                        )
                    })),
                )
                .child(
                    settings_nav_row(
                        "settings-system-prompts",
                        AppIcon::MessageText,
                        "System Prompts",
                        "Default instructions",
                        prompts_selected,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.request_select_settings_section(
                            SettingsSection::SystemPrompts,
                            window,
                            cx,
                        )
                    })),
                )
                .child(
                    settings_nav_row(
                        "settings-mcp",
                        AppIcon::Plug,
                        "MCP Servers",
                        "Local tools over stdio",
                        mcp_selected,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.request_select_settings_section(SettingsSection::Mcp, window, cx)
                    })),
                ),
        )
        .child(
            div()
                .px_5()
                .pt_6()
                .pb_2()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child("Providers"),
                )
                .child(
                    Compact
                        .icon_action(
                            "add-provider-sidebar",
                            AppIcon::Plus,
                            IconTone::Accent,
                            "Add provider",
                            cx,
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| {
                                this.request_add_provider(window, cx)
                            }),
                        ),
                ),
        )
        .child(
            div()
                .id("settings-provider-list")
                .min_h_0()
                .flex_1()
                .overflow_y_scroll()
                .px_3()
                .child(providers),
        )
        .child(
            div().flex_none().p_2().child(
                Button::new("back-to-chats")
                    .ghost()
                    .w_full()
                    .h(px(36.0))
                    .px_2p5()
                    .rounded(px(9.0))
                    .tooltip("Back to chats")
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(render_icon(
                                        AppIcon::MessageText,
                                        IconTone::Muted,
                                        16.0,
                                        cx,
                                    ))
                                    .child("Chats"),
                            )
                            .child(render_icon(
                                AppIcon::ChevronRight,
                                IconTone::Muted,
                                14.0,
                                cx,
                            )),
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| this.request_leave_settings(window, cx)),
                    ),
            ),
        )
        .into_any_element()
}

fn settings_nav_row(
    id: impl Into<ElementId>,
    icon: AppIcon,
    title: &'static str,
    detail: &'static str,
    selected: bool,
    cx: &App,
) -> Stateful<Div> {
    let icon = render_icon(
        icon,
        if selected {
            IconTone::Accent
        } else {
            IconTone::Muted
        },
        18.0,
        cx,
    );
    let accent = cx.theme().accent;
    let hover = cx.theme().list_hover;

    div()
        .id(id)
        .rounded(px(10.0))
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .gap_3()
        .bg(if selected {
            accent
        } else {
            cx.theme().transparent
        })
        .cursor_pointer()
        .hover(move |style| style.bg(if selected { accent } else { hover }))
        .active(move |style| style.bg(accent))
        .child(
            div()
                .w(px(22.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(icon),
        )
        .child(
            div()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .font_weight(if selected {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .child(title),
                )
                .child(
                    div()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(detail),
                ),
        )
}

fn provider_nav_row(
    provider: &Provider,
    index: usize,
    model_count: usize,
    selected: bool,
    drop_after: Option<bool>,
    cx: &mut Context<OneChat>,
) -> Stateful<Div> {
    let select_id = provider.id.clone();
    let toggle_id = provider.id.clone();
    let drag_target_id = provider.id.clone();
    let drop_target_id = provider.id.clone();
    let app = cx.entity().downgrade();
    let drag = ProviderDrag {
        id: provider.id.clone(),
        name: provider.name.clone().into(),
    };
    let accent = cx.theme().accent;
    let hover = cx.theme().list_hover;
    let indicator = drop_after.map(|after| {
        let line = div()
            .absolute()
            .left_2()
            .right_2()
            .h(px(2.0))
            .rounded_full()
            .bg(cx.theme().primary);
        if after {
            line.bottom(px(-3.0))
        } else {
            line.top(px(-3.0))
        }
    });

    div()
        .id(SharedString::from(format!(
            "settings-provider-{}",
            provider.id
        )))
        .relative()
        .rounded(px(10.0))
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .gap_3()
        .bg(if selected {
            accent
        } else {
            cx.theme().transparent
        })
        .cursor_grab()
        .hover(move |style| style.bg(if selected { accent } else { hover }))
        .children(indicator)
        .child(
            Switch::new(SharedString::from(format!(
                "toggle-provider-sidebar-{}",
                provider.id
            )))
            .small()
            .checked(provider.enabled)
            .color(cx.theme().primary)
            .tooltip(if provider.enabled {
                "Disable provider"
            } else {
                "Enable provider"
            })
            .on_click(cx.listener(move |this, enabled: &bool, _, cx| {
                this.set_provider_enabled(toggle_id.clone(), *enabled, cx)
            })),
        )
        .child(
            div()
                .id(SharedString::from(format!(
                    "select-provider-sidebar-{}",
                    provider.id
                )))
                .min_w_0()
                .flex_1()
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.request_select_settings_section(
                        SettingsSection::Provider(select_id.clone()),
                        window,
                        cx,
                    )
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_sm()
                                .font_weight(if selected {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .child(provider.name.clone()),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_size(px(11.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "{} {}",
                                    model_count,
                                    if model_count == 1 { "model" } else { "models" }
                                )),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(provider.kind.label()),
                ),
        )
        .on_drag(drag, move |drag, _, _, cx| {
            _ = app.update(cx, |this, cx| {
                if this.settings_ui.provider_drop_target.take().is_some() {
                    cx.notify();
                }
            });
            cx.new(|_| drag.clone())
        })
        .on_drag_move(
            cx.listener(move |this, event: &DragMoveEvent<ProviderDrag>, _, cx| {
                let after = if event.bounds.contains(&event.event.position) {
                    let after = event.event.position.y >= event.bounds.center().y;
                    let from = this
                        .data
                        .snapshot
                        .providers
                        .iter()
                        .position(|provider| provider.id == event.drag(cx).id);
                    let gap = index + usize::from(after);
                    from.filter(|from| gap != *from && gap != *from + 1)
                        .map(|_| after)
                } else {
                    None
                };
                this.set_provider_drop_target(drag_target_id.clone(), after, cx);
            }),
        )
        .on_drop(cx.listener(move |this, drag: &ProviderDrag, _, cx| {
            let Some((target_id, after)) = this
                .settings_ui
                .provider_drop_target
                .clone()
                .filter(|(target_id, _)| target_id == &drop_target_id)
            else {
                return;
            };
            this.reorder_provider(drag.id.clone(), target_id, after, cx);
        }))
}
