use super::*;

mod attachments;
mod controls;
mod recording;

use attachments::render_attachments;
use controls::{
    render_add_action, render_expand_action, render_microphone_action, render_primary_action,
};
use recording::render_recording_status;

pub(super) fn render_composer(
    app: &OneChat,
    message_max_width: f32,
    typography: MessageTypography,
    context_usage_popover_progress: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let generating = app.is_current_generating();
    let recording_status = app.chat.audio_recording.status;
    let recording_active = recording_status.is_active();
    let show_microphone = recording_active
        || app
            .current_model()
            .is_some_and(|model| model.capabilities.audio_input);
    let multiline = composer_is_multiline(app, message_max_width, cx);
    let expanded = multiline && app.chat.composer_expanded.get();
    let show_context_indicator = app.current_model().is_some();
    let context_indicator = show_context_indicator.then(|| {
        div()
            .absolute()
            .right(px(if multiline { 91.0 } else { 49.0 }))
            .bottom(px(7.0))
            .child(render_context_indicator(
                app,
                context_usage_popover_progress,
                cx,
            ))
    });
    let can_send = (!app.chat.composer.read(cx).value().trim().is_empty()
        || !app.chat.attachments.is_empty())
        && !app.chat.attachments_loading
        && !recording_active
        && app.attachment_context_supported()
        && app.current_model().is_some()
        && app.current_conversation().is_some();
    let action = render_primary_action(generating, recording_active, can_send, cx);
    let expand = render_expand_action(multiline, expanded, recording_active, cx);
    let add = render_add_action(
        generating,
        recording_active,
        app.chat.attachments_loading,
        cx,
    );
    let microphone = render_microphone_action(app, show_microphone, recording_status, cx);
    let attachments = render_attachments(app, cx);

    let input = div()
        .relative()
        .min_w_0()
        .flex_1()
        .overflow_hidden()
        .rounded(px(22.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .shadow_md()
        .capture_action(cx.listener(|this, _: &Paste, _, cx| this.paste_composer_image(cx)))
        .capture_action(cx.listener(|this, _: &InputEscape, _, cx| {
            if this.recording_active() {
                cx.stop_propagation();
                this.cancel_voice_recording(cx);
            }
        }))
        .capture_action(cx.listener(|this, action: &Enter, window, cx| {
            if this.recording_active() {
                cx.stop_propagation();
                this.stop_voice_recording(cx);
                return;
            }
            let send = !action.shift
                && match this.settings().send_message_shortcut {
                    SendMessageShortcut::Enter => !action.secondary,
                    SendMessageShortcut::SecondaryEnter => action.secondary,
                };
            if send {
                cx.stop_propagation();
                this.send_composer(window, cx);
            }
        }))
        .children(attachments)
        .child(if recording_active {
            div()
                .relative()
                .w_full()
                .h(px(48.0))
                .child(
                    Input::new(&app.chat.composer)
                        .aria_label("Message")
                        .appearance(false)
                        .w_full()
                        .py(px(12.0)),
                )
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .overflow_hidden()
                        .rounded(px(21.0))
                        .bg(cx.theme().popover)
                        .child(render_recording_status(app, cx)),
                )
                .into_any_element()
        } else {
            let input = Input::new(&app.chat.composer)
                .aria_label("Message")
                .appearance(false)
                .text_size(px(typography.body_size))
                .line_height(px(typography.body_line_height));

            if multiline {
                let input = input.px(px(12.0)).py(px(0.0));
                let input = if expanded { input.h(px(480.0)) } else { input };
                div()
                    .w_full()
                    .min_w_0()
                    .pt(px(12.0))
                    .pr(px(4.0))
                    .child(input)
                    .into_any_element()
            } else {
                input
                    .pl(px(if show_microphone { 98.0 } else { 56.0 }))
                    .pr(px(if show_context_indicator { 98.0 } else { 56.0 }))
                    .py(px(12.0))
                    .into_any_element()
            }
        })
        .children((multiline && !recording_active).then(|| div().h(px(48.0)).flex_none()))
        .child(add)
        .children(microphone)
        .child(action)
        .children(expand)
        .children(context_indicator);

    div()
        .flex_none()
        .w_full()
        .px_6()
        .pt_2()
        .pb_4()
        .child(
            div()
                .mx_auto()
                .w_full()
                .max_w(px(message_max_width))
                .child(input),
        )
        .into_any_element()
}

fn composer_is_multiline(app: &OneChat, width: f32, cx: &App) -> bool {
    let composer = app.chat.composer.read(cx);
    let value = composer.value();
    if value.is_empty() {
        app.chat.composer_multiline.set(false);
        app.chat.composer_expanded.set(false);
        return false;
    }
    if value.contains('\n') {
        app.chat.composer_multiline.set(true);
        return true;
    }

    let Some((bounds, line_height)) = composer
        .range_to_bounds(&(0..value.len()))
        .zip(composer.line_height())
    else {
        return app.chat.composer_multiline.get();
    };
    let multiline = bounds.size.height > line_height + px(0.5)
        || bounds.size.width > px((width - 112.0).max(0.0));
    app.chat.composer_multiline.set(multiline);
    if !multiline {
        app.chat.composer_expanded.set(false);
    }
    multiline
}
