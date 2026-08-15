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
    let show_context_indicator = app.current_model().is_some();
    let multiline = composer_is_multiline(app, cx);
    let expanded = multiline && app.chat.composer_expanded.get();
    let can_send = (!app.chat.composer.read(cx).value().trim().is_empty()
        || !app.chat.attachments.is_empty())
        && !app.chat.attachments_loading
        && !recording_active
        && app.attachment_context_supported()
        && app.current_model().is_some()
        && app.current_conversation().is_some();
    let attachments = render_attachments(app, cx);
    let add = render_add_action(
        generating,
        recording_active,
        app.chat.attachments_loading,
        cx,
    );
    let microphone = render_microphone_action(app, show_microphone, recording_status, cx);
    let left_actions = div()
        .flex_none()
        .flex()
        .items_center()
        .gap_2()
        .child(add)
        .children(microphone);

    let context_indicator = show_context_indicator
        .then(|| render_context_indicator(app, context_usage_popover_progress, cx));
    let expand = render_expand_action(multiline, expanded, recording_active, cx);
    let action = render_primary_action(generating, recording_active, can_send, cx);
    let right_actions = div()
        .flex_none()
        .flex()
        .items_center()
        .gap_2()
        .children(context_indicator)
        .children(expand)
        .child(action);

    let editor = if recording_active {
        div()
            .relative()
            .min_w_0()
            .flex_1()
            .child(
                Textarea::new(&app.chat.composer)
                    .aria_label("Message")
                    .appearance(false)
                    .absolute()
                    .inset_0(),
            )
            .child(
                div()
                    .relative()
                    .w_full()
                    .bg(cx.theme().popover)
                    .child(render_recording_status(app, cx)),
            )
            .into_any_element()
    } else {
        let input = Textarea::new(&app.chat.composer)
            .aria_label("Message")
            .appearance(false)
            .w_full()
            .text_size(px(typography.body_size))
            .line_height(px(typography.body_line_height));
        let input = if expanded { input.h(px(480.0)) } else { input };
        div().min_w_0().flex_1().child(input).into_any_element()
    };

    let editor_row = div()
        .min_w_0()
        .w_full()
        .px(px(7.0))
        .flex()
        .gap_2()
        .child(left_actions)
        .child(editor)
        .child(right_actions);
    let editor_row = if multiline && !recording_active {
        editor_row.py(px(7.0)).items_end()
    } else {
        editor_row.h(px(48.0)).items_center()
    };

    let input = div()
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
        .child(editor_row);

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

fn composer_is_multiline(app: &OneChat, cx: &App) -> bool {
    let current = app.chat.composer_multiline.get();
    let composer = app.chat.composer.read(cx);
    let value = composer.value();

    // Preedit changes are not InputEvent::Change events. Keep the outer layout fixed while the
    // IME owns temporary text, otherwise every candidate update can move its own anchor.
    if value != app.chat.composer_committed_value {
        return current;
    }
    if value.is_empty() {
        app.chat.composer_multiline.set(false);
        app.chat.composer_expanded.set(false);
        return false;
    }
    if value.contains('\n') {
        app.chat.composer_multiline.set(true);
        return true;
    }

    let Some(text_bounds) = composer.range_to_bounds(&(0..value.len())) else {
        return current;
    };
    let Some(line_height) = composer.line_height() else {
        return current;
    };
    let multiline = text_bounds.size.height > line_height + px(0.5);

    app.chat.composer_multiline.set(multiline);
    if !multiline {
        app.chat.composer_expanded.set(false);
    }
    multiline
}
