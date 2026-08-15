use super::super::super::*;

pub(super) fn default_prompt_select(app: &OneChat) -> AnyElement {
    Select::new(&app.settings_ui.default_prompt_select)
        .large()
        .h(px(40.0))
        .px(px(12.0))
        .rounded(px(10.0))
        .placeholder("No Prompt Preset")
        .menu_max_h(px(320.0))
        .w(px(300.0))
        .into_any_element()
}

pub(super) fn prompt_presets_content(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    if app.data.snapshot.prompt_presets.is_empty() {
        return div()
            .w_full()
            .px_4()
            .py_6()
            .flex()
            .flex_col()
            .items_center()
            .text_center()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("No presets yet"),
            )
            .child(
                div()
                    .pt_1()
                    .text_size(px(12.0))
                    .text_color(cx.theme().muted_foreground)
                    .child("Create reusable conversation setups for new chats."),
            )
            .into_any_element();
    }

    div()
        .w_full()
        .flex()
        .flex_col()
        .children(app.data.snapshot.prompt_presets.iter().map(|preset| {
            let view_name = preset.name.clone();
            let edit_name = preset.name.clone();
            let delete_name = preset.name.clone();
            let default =
                app.settings().default_prompt_preset.as_deref() == Some(preset.name.as_str());
            div()
                .id(SharedString::from(format!(
                    "prompt-preset-card-{}",
                    preset.name
                )))
                .w_full()
                .min_h(px(68.0))
                .rounded_lg()
                .px_3()
                .py_2()
                .flex()
                .items_center()
                .gap_3()
                .hover(|style| style.bg(cx.theme().list_hover))
                .child(
                    div()
                        .size(px(32.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(9.0))
                        .bg(cx.theme().muted)
                        .child(render_icon(AppIcon::FileText, IconTone::Muted, 16.0, cx)),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
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
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(preset.name.clone()),
                                )
                                .children(default.then(|| {
                                    status_pill("Default", true, StatusPillBackground::Muted, cx)
                                }))
                                .children((!preset.assistant_opening.is_empty()).then(|| {
                                    status_pill(
                                        "Assistant Opening",
                                        true,
                                        StatusPillBackground::Background,
                                        cx,
                                    )
                                })),
                        )
                        .child(
                            div()
                                .pt_1()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(text_summary(&preset.system_prompt, 420, None)),
                        ),
                )
                .child(
                    Compact
                        .icon_action(
                            SharedString::from(format!("view-prompt-{}", preset.name)),
                            AppIcon::Eye,
                            IconTone::Muted,
                            "View preset",
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.view_prompt_preset(view_name.clone(), window, cx)
                        })),
                )
                .child(
                    Compact
                        .icon_action(
                            SharedString::from(format!("edit-prompt-{}", preset.name)),
                            AppIcon::Pencil,
                            IconTone::Muted,
                            "Edit preset",
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.begin_edit_prompt_preset(edit_name.clone(), window, cx)
                        })),
                )
                .child(
                    Compact
                        .icon_action(
                            SharedString::from(format!("delete-prompt-{}", preset.name)),
                            AppIcon::Trash,
                            IconTone::Danger,
                            "Delete preset",
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.request_delete_prompt_preset(delete_name.clone(), window, cx)
                        })),
                )
        }))
        .into_any_element()
}
