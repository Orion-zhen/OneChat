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
        .w_full()
        .h(px(52.0))
        .flex_none()
        .px(px(12.0))
        .flex()
        .items_center()
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
                .flex_none()
                .on_click(move |_, window, cx| {
                    close_app.update(cx, |app, cx| app.cancel_prompt_variable_edit(cx));
                    window.close_dialog(cx);
                }),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .text_center()
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            Compact
                .primary_icon_action("save-prompt-variable", AppIcon::Save, "Save", cx)
                .flex_none()
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
