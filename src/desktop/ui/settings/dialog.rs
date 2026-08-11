use super::*;

pub(crate) fn prompt_variable_dialog(
    dialog: Dialog,
    app: Entity<OneChat>,
    _window: &mut Window,
    cx: &mut App,
) -> Dialog {
    let state = app.read(cx);
    let title = if state
        .settings_ui
        .prompt_variable_editor
        .as_ref()
        .and_then(|editor| editor.original_name())
        .is_some()
    {
        "Edit prompt variable"
    } else {
        "New prompt variable"
    };
    let body = prompt_variable_dialog_body(state, app.clone(), cx);
    let cancel_app = app.clone();
    let close_app = app.clone();
    let save_app = app.clone();
    let header = div()
        .relative()
        .w_full()
        .h(px(52.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            Compact
                .icon_action(
                    "close-prompt-variable",
                    AppIcon::Close,
                    IconTone::Muted,
                    "Cancel",
                    cx,
                )
                .absolute()
                .left(px(12.0))
                .top(px(11.0))
                .on_click(move |_, window, cx| {
                    close_app.update(cx, |app, cx| app.cancel_prompt_variable_edit(cx));
                    window.close_dialog(cx);
                }),
        )
        .child(
            div()
                .px(px(52.0))
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            Compact
                .primary_icon_action("save-prompt-variable", AppIcon::Save, "Save", cx)
                .absolute()
                .right(px(12.0))
                .top(px(11.0))
                .on_click(move |_, window, cx| {
                    if save_app.update(cx, |app, cx| app.save_prompt_variable(cx)) {
                        window.close_dialog(cx);
                    }
                }),
        );

    let ok_app = app;
    dialog
        .width(px(600.0))
        .margin_top(px(56.0))
        .p_0()
        .rounded(px(18.0))
        .bg(cx.theme().popover)
        .close_button(false)
        .title(header)
        .child(body)
        .on_cancel(move |_, _, cx| {
            cancel_app.update(cx, |app, cx| app.cancel_prompt_variable_edit(cx));
            true
        })
        .on_ok(move |_, _, cx| ok_app.update(cx, |app, cx| app.save_prompt_variable(cx)))
}

pub(crate) fn prompt_preset_dialog(
    dialog: Dialog,
    app: Entity<OneChat>,
    _window: &mut Window,
    cx: &mut App,
) -> Dialog {
    let state = app.read(cx);
    let editing = state.settings_ui.prompt_preset_editor.is_some();
    let title =
        state
            .settings_ui
            .prompt_preset_editor
            .as_ref()
            .map_or("View prompt preset", |editor| {
                if editor.original_name().is_some() {
                    "Edit prompt preset"
                } else {
                    "New prompt preset"
                }
            });
    let body = prompt_preset_dialog_body(state, cx);

    let cancel_app = app.clone();
    let close_app = app.clone();
    let header = div()
        .relative()
        .w_full()
        .h(px(52.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            Compact
                .icon_action(
                    "close-prompt-preset",
                    AppIcon::Close,
                    IconTone::Muted,
                    if editing { "Cancel" } else { "Close" },
                    cx,
                )
                .absolute()
                .left(px(12.0))
                .top(px(11.0))
                .on_click(move |_, window, cx| {
                    close_app.update(cx, |app, cx| {
                        if editing {
                            app.cancel_prompt_preset_edit(cx);
                        } else {
                            app.settings_ui.viewed_prompt_preset = None;
                            app.settings_ui.form_error = None;
                            cx.notify();
                        }
                    });
                    window.close_dialog(cx);
                }),
        )
        .child(
            div()
                .px(px(52.0))
                .text_size(px(15.0))
                .line_height(px(20.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .when(editing, |header| {
            let save_app = app.clone();
            header.child(
                Compact
                    .primary_icon_action("save-prompt-preset", AppIcon::Save, "Save", cx)
                    .absolute()
                    .right(px(12.0))
                    .top(px(11.0))
                    .on_click(move |_, window, cx| {
                        let saved = save_app.update(cx, |app, cx| app.save_prompt_preset(cx));
                        if saved {
                            window.close_dialog(cx);
                        }
                    }),
            )
        });

    let mut dialog = dialog
        .width(px(560.0))
        .margin_top(px(56.0))
        .p_0()
        .rounded(px(18.0))
        .bg(cx.theme().popover)
        .close_button(false)
        .title(header)
        .child(body)
        .on_cancel(move |_, _, cx| {
            cancel_app.update(cx, |app, cx| {
                if editing {
                    app.cancel_prompt_preset_edit(cx);
                } else {
                    app.settings_ui.viewed_prompt_preset = None;
                    app.settings_ui.form_error = None;
                    cx.notify();
                }
            });
            true
        });

    if editing {
        let save_app = app;
        dialog =
            dialog.on_ok(move |_, _, cx| save_app.update(cx, |app, cx| app.save_prompt_preset(cx)));
    }
    dialog
}
