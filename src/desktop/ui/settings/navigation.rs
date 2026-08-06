use super::*;

pub(super) fn settings_sidebar(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let general_selected = app.settings_ui.section == SettingsSection::General;
    let default_models_selected = app.settings_ui.section == SettingsSection::DefaultModels;
    let prompts_selected = app.settings_ui.section == SettingsSection::SystemPrompts;
    let mut providers = div().flex().flex_col().gap_1();

    for provider in &app.data.snapshot.providers {
        let provider_id = provider.id.clone();
        let selected = app.settings_ui.section == SettingsSection::Provider(provider.id.clone());
        let model_count = app
            .data
            .snapshot
            .models
            .iter()
            .filter(|model| model.provider_id == provider.id)
            .count();
        let status_color = match app.settings_ui.connection_tests.get(&provider.id) {
            Some(ConnectionTestStatus::Testing) => colors.accent,
            Some(ConnectionTestStatus::Connected) => colors.success,
            Some(ConnectionTestStatus::Failed(_)) => colors.danger,
            None if provider.enabled => colors.success,
            None => colors.muted,
        };
        providers = providers.child(
            provider_nav_row(provider, model_count, status_color, selected, colors).on_click(
                cx.listener(move |this, _, _, cx| {
                    this.select_settings_section(SettingsSection::Provider(provider_id.clone()), cx)
                }),
            ),
        );
    }

    if app.data.snapshot.providers.is_empty() {
        providers = providers.child(
            div()
                .px_3()
                .py_2()
                .text_size(px(12.0))
                .text_color(colors.muted)
                .child("No providers configured"),
        );
    }

    div()
        .w(px(244.0))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(colors.border)
        .bg(colors.sidebar)
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
                        SettingsNavIcon::Text("⚙"),
                        "General",
                        "Appearance and behavior",
                        general_selected,
                        colors,
                        scale_factor,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_settings_section(SettingsSection::General, cx)
                    })),
                )
                .child(
                    settings_nav_row(
                        "settings-default-models",
                        SettingsNavIcon::Text("◇"),
                        "Default Models",
                        "Models for new conversations",
                        default_models_selected,
                        colors,
                        scale_factor,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_settings_section(SettingsSection::DefaultModels, cx)
                    })),
                )
                .child(
                    settings_nav_row(
                        "settings-system-prompts",
                        SettingsNavIcon::Text("✦"),
                        "System Prompts",
                        "Default instructions",
                        prompts_selected,
                        colors,
                        scale_factor,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_settings_section(SettingsSection::SystemPrompts, cx)
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
                        .text_color(colors.muted)
                        .child("PROVIDERS"),
                )
                .child(
                    svg_icon_button(
                        "add-provider-sidebar",
                        UiIcon::Plus,
                        IconTone::Muted,
                        colors,
                        scale_factor,
                    )
                    .size(px(26.0))
                    .on_click(cx.listener(|this, _, _, cx| this.begin_add_provider(cx))),
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
            div().border_t_1().border_color(colors.border).p_3().child(
                settings_nav_row(
                    "settings-add-provider",
                    SettingsNavIcon::Svg(UiIcon::Plus),
                    "Add Provider",
                    "Connect another service",
                    app.settings_ui.section == SettingsSection::NewProvider,
                    colors,
                    scale_factor,
                )
                .on_click(cx.listener(|this, _, _, cx| this.begin_add_provider(cx))),
            ),
        )
        .into_any_element()
}

enum SettingsNavIcon {
    Text(&'static str),
    Svg(UiIcon),
}

fn settings_nav_row(
    id: impl Into<ElementId>,
    icon: SettingsNavIcon,
    title: &'static str,
    detail: &'static str,
    selected: bool,
    colors: Colors,
    scale_factor: f32,
) -> Stateful<Div> {
    let icon = match icon {
        SettingsNavIcon::Text(icon) => div().child(icon).into_any_element(),
        SettingsNavIcon::Svg(icon) => svg_icon(
            icon,
            if selected {
                IconTone::Accent
            } else {
                IconTone::Muted
            },
            colors,
            scale_factor,
            16.0,
        ),
    };

    div()
        .id(id)
        .rounded_lg()
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .gap_3()
        .bg(if selected {
            colors.accent_soft
        } else {
            rgba(0x00000000)
        })
        .cursor_pointer()
        .hover(move |style| {
            style.bg(if selected {
                colors.accent_soft
            } else {
                colors.hover
            })
        })
        .active(move |style| style.bg(colors.accent_soft))
        .child(
            div()
                .w(px(22.0))
                .flex_none()
                .text_center()
                .text_size(px(15.0))
                .text_color(if selected {
                    colors.accent
                } else {
                    colors.muted
                })
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
                        .text_color(colors.muted)
                        .child(detail),
                ),
        )
}

fn provider_nav_row(
    provider: &Provider,
    model_count: usize,
    status_color: gpui::Rgba,
    selected: bool,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!(
            "settings-provider-{}",
            provider.id
        )))
        .rounded_lg()
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .gap_3()
        .bg(if selected {
            colors.accent_soft
        } else {
            rgba(0x00000000)
        })
        .cursor_pointer()
        .hover(move |style| {
            style.bg(if selected {
                colors.accent_soft
            } else {
                colors.hover
            })
        })
        .active(move |style| style.bg(colors.accent_soft))
        .child(
            div()
                .w(px(22.0))
                .flex_none()
                .flex()
                .justify_center()
                .child(div().size(px(8.0)).rounded_full().bg(status_color)),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .child(
                    div()
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
                        .text_size(px(11.0))
                        .text_color(colors.muted)
                        .child(format!(
                            "{} · {} {}",
                            provider.kind.label(),
                            model_count,
                            if model_count == 1 { "model" } else { "models" }
                        )),
                ),
        )
}
