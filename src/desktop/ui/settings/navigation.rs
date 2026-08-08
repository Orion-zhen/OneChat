use super::*;

pub(super) fn settings_sidebar(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let general_selected = app.settings_ui.section == SettingsSection::General;
    let default_models_selected = app.settings_ui.section == SettingsSection::DefaultModels;
    let prompts_selected = app.settings_ui.section == SettingsSection::SystemPrompts;
    let mcp_selected = app.settings_ui.section == SettingsSection::Mcp;
    let mut providers = div().flex().flex_col().gap_1();

    for provider in &app.data.snapshot.providers {
        let selected = app.settings_ui.section == SettingsSection::Provider(provider.id.clone());
        let model_count = app
            .data
            .snapshot
            .models
            .iter()
            .filter(|model| model.provider_id == provider.id)
            .count();
        providers = providers.child(provider_nav_row(provider, model_count, selected, cx));
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
        .w(px(SIDEBAR_WIDTH))
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
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_settings_section(SettingsSection::General, cx)
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
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_settings_section(SettingsSection::DefaultModels, cx)
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
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_settings_section(SettingsSection::SystemPrompts, cx)
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
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_settings_section(SettingsSection::Mcp, cx)
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
                    icon_action(
                        "add-provider-sidebar",
                        AppIcon::Plus,
                        IconTone::Accent,
                        "Add provider",
                        cx,
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| this.begin_add_provider(window, cx)),
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
                    .h(px(34.0))
                    .px_2()
                    .rounded(px(7.0))
                    .tooltip("Back to chats")
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(render_icon(AppIcon::MessageText, IconTone::Muted, 16.0, cx))
                            .child("Chats"),
                    )
                    .child(render_icon(
                        AppIcon::ChevronRight,
                        IconTone::Muted,
                        14.0,
                        cx,
                    ))
                    .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Chat, cx))),
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
    model_count: usize,
    selected: bool,
    cx: &mut Context<OneChat>,
) -> Stateful<Div> {
    let select_id = provider.id.clone();
    let toggle_id = provider.id.clone();
    let accent = cx.theme().accent;
    let hover = cx.theme().list_hover;

    div()
        .id(SharedString::from(format!(
            "settings-provider-{}",
            provider.id
        )))
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
        .hover(move |style| style.bg(if selected { accent } else { hover }))
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
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_settings_section(SettingsSection::Provider(select_id.clone()), cx)
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
}
