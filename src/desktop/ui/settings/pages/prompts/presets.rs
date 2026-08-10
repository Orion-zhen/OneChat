use super::super::super::*;
use super::components::{field_label, readonly_field};

pub(super) fn default_prompt_select(app: &OneChat) -> AnyElement {
    Select::new(&app.settings_ui.default_prompt_select)
        .large()
        .h(px(40.0))
        .px(px(12.0))
        .rounded(px(10.0))
        .placeholder("No System Prompt")
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
                    .child("No prompts yet"),
            )
            .child(
                div()
                    .pt_1()
                    .text_size(px(12.0))
                    .text_color(cx.theme().muted_foreground)
                    .child("Create reusable instructions for new conversations."),
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
            let default = app.settings().default_system_prompt_preset.as_deref()
                == Some(preset.name.as_str());
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
                                .children(default.then(|| status_pill("Default", true, cx))),
                        )
                        .child(
                            div()
                                .pt_1()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(prompt_preview(&preset.content)),
                        ),
                )
                .child(
                    icon_action(
                        SharedString::from(format!("view-prompt-{}", preset.name)),
                        AppIcon::Eye,
                        IconTone::Muted,
                        "View prompt",
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.view_prompt_preset(view_name.clone(), window, cx)
                    })),
                )
                .child(
                    icon_action(
                        SharedString::from(format!("edit-prompt-{}", preset.name)),
                        AppIcon::Pencil,
                        IconTone::Muted,
                        "Edit prompt",
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.begin_edit_prompt_preset(edit_name.clone(), window, cx)
                    })),
                )
                .child(
                    icon_action(
                        SharedString::from(format!("delete-prompt-{}", preset.name)),
                        AppIcon::Trash,
                        IconTone::Danger,
                        "Delete prompt",
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.request_delete_prompt_preset(delete_name.clone(), window, cx)
                    })),
                )
        }))
        .into_any_element()
}

fn prompt_preset_field(label: &'static str, input: &Entity<InputState>, multiline: bool) -> Field {
    Field::new().label(label).required(true).child(
        Input::new(input)
            .aria_label(label)
            .large()
            .rounded(px(12.0))
            .when(multiline, |input| input.h(px(240.0))),
    )
}

pub(in crate::desktop::ui::settings) fn prompt_preset_dialog_body(
    app: &OneChat,
    cx: &App,
) -> AnyElement {
    if let Some(editor) = &app.settings_ui.prompt_preset_editor {
        return stretching_column()
            .px_5()
            .pb_5()
            .gap_3()
            .child(
                Form::vertical()
                    .child(prompt_preset_field("Name", &editor.name, false))
                    .child(prompt_preset_field("Prompt", &editor.content, true)),
            )
            .children(app.settings_ui.form_error.as_deref().map(error_banner))
            .into_any_element();
    }

    let preset = app
        .settings_ui
        .viewed_prompt_preset
        .as_deref()
        .and_then(|name| app.prompt_preset(name))
        .expect("prompt preset dialog requires a viewed preset or editor");
    stretching_column()
        .px_5()
        .pb_5()
        .gap_4()
        .child(readonly_field("Name", preset.name.clone(), cx))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(field_label("Prompt", cx))
                .child(
                    div()
                        .id("viewed-prompt-content")
                        .min_h(px(200.0))
                        .max_h(px(300.0))
                        .overflow_y_scroll()
                        .rounded_lg()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().muted)
                        .p_3()
                        .whitespace_normal()
                        .text_sm()
                        .line_height(px(22.0))
                        .child(preset.content.clone()),
                ),
        )
        .into_any_element()
}
