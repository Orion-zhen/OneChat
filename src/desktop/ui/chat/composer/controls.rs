use super::*;
use crate::desktop::audio_recording::RecordingStatus;

pub(super) fn render_primary_action(
    generating: bool,
    recording_active: bool,
    can_send: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    if generating {
        Button::new("composer-stop")
            .danger()
            .bg(cx.theme().danger)
            .rounded(px(17.0))
            .tooltip("Stop generating")
            .size(px(34.0))
            .p_0()
            .absolute()
            .right(px(7.0))
            .bottom(px(7.0))
            .child(render_icon(AppIcon::Stop, IconTone::OnAccent, 16.0, cx))
            .on_click(cx.listener(|this, _, _, cx| this.stop_current_generation(cx)))
            .into_any_element()
    } else if recording_active {
        Button::new("composer-cancel-recording")
            .ghost()
            .rounded(px(17.0))
            .tooltip("Cancel recording")
            .size(px(34.0))
            .p_0()
            .absolute()
            .right(px(7.0))
            .bottom(px(7.0))
            .child(render_icon(AppIcon::Close, IconTone::Muted, 17.0, cx))
            .on_click(cx.listener(|this, _, _, cx| this.cancel_voice_recording(cx)))
            .into_any_element()
    } else {
        Button::new("composer-send")
            .primary()
            .rounded(px(17.0))
            .tooltip("Send message")
            .disabled(!can_send)
            .size(px(34.0))
            .p_0()
            .absolute()
            .right(px(7.0))
            .bottom(px(7.0))
            .child(render_icon(
                AppIcon::ArrowUp,
                if can_send {
                    IconTone::OnAccent
                } else {
                    IconTone::Muted
                },
                20.0,
                cx,
            ))
            .on_click(cx.listener(|this, _, window, cx| this.send_composer(window, cx)))
            .into_any_element()
    }
}

pub(super) fn render_expand_action(
    multiline: bool,
    expanded: bool,
    recording_active: bool,
    cx: &mut Context<OneChat>,
) -> Option<AnyElement> {
    (multiline && !recording_active).then(|| {
        Button::new("composer-expand")
            .ghost()
            .rounded(px(17.0))
            .tooltip(if expanded {
                "Collapse input"
            } else {
                "Expand input"
            })
            .size(px(34.0))
            .p_0()
            .absolute()
            .right(px(49.0))
            .bottom(px(7.0))
            .child(render_icon(
                if expanded {
                    AppIcon::Minimize
                } else {
                    AppIcon::Maximize
                },
                IconTone::Muted,
                18.0,
                cx,
            ))
            .on_click(cx.listener(|this, _, window, cx| {
                this.chat
                    .composer_expanded
                    .set(!this.chat.composer_expanded.get());
                let composer = this.chat.composer.clone();
                let selection = composer.read(cx).selected_range();
                composer.update(cx, |composer, cx| composer.focus(window, cx));
                cx.on_next_frame(window, move |_, window, cx| {
                    cx.on_next_frame(window, move |_, window, cx| {
                        composer.update(cx, |composer, cx| {
                            composer.set_selected_range(selection, cx);
                            composer.focus(window, cx);
                        });
                    });
                });
                cx.notify();
            }))
            .into_any_element()
    })
}

pub(super) fn render_add_action(
    generating: bool,
    recording_active: bool,
    attachments_loading: bool,
    cx: &mut Context<OneChat>,
) -> Button {
    Button::new("composer-add-attachment")
        .ghost()
        .rounded(px(17.0))
        .tooltip(if attachments_loading {
            "Loading attachments"
        } else {
            "Add attachments"
        })
        .disabled(generating || recording_active || attachments_loading)
        .size(px(34.0))
        .p_0()
        .absolute()
        .left(px(7.0))
        .bottom(px(7.0))
        .child(render_icon(AppIcon::Plus, IconTone::Muted, 20.0, cx))
        .on_click(cx.listener(|this, _, _, cx| this.add_attachments(cx)))
}

pub(super) fn render_microphone_action(
    app: &OneChat,
    show: bool,
    status: RecordingStatus,
    cx: &mut Context<OneChat>,
) -> Option<AnyElement> {
    show.then(|| {
        let can_start = app.can_start_voice_recording();
        let recording = status == RecordingStatus::Recording;
        let busy = matches!(
            status,
            RecordingStatus::RequestingPermission | RecordingStatus::Finalizing
        );
        let tooltip = if recording {
            "Stop recording"
        } else if busy {
            "Preparing voice message"
        } else if can_start {
            "Record voice message"
        } else {
            "Voice recording is unavailable right now"
        };
        let button = Button::new("composer-microphone")
            .rounded(px(17.0))
            .tooltip(tooltip)
            .disabled(!recording && !can_start)
            .size(px(34.0))
            .p_0()
            .absolute()
            .left(px(49.0))
            .bottom(px(7.0))
            .child(render_icon(
                if recording {
                    AppIcon::Stop
                } else {
                    AppIcon::Mic
                },
                if recording {
                    IconTone::OnAccent
                } else if can_start {
                    IconTone::Accent
                } else {
                    IconTone::Muted
                },
                if recording { 15.0 } else { 18.0 },
                cx,
            ))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_voice_recording(cx)));
        if recording {
            button.danger().bg(cx.theme().danger).into_any_element()
        } else {
            button.ghost().into_any_element()
        }
    })
}
