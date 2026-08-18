use gpui::{AnyElement, Context, Focusable as _, MouseButton, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme as _, Icon, IconName,
    button::{Button, ButtonVariants as _},
    input::Textarea,
};

use crate::desktop::{
    app::{OneChat, TranslationPromptKind},
    ui::{
        icons::{AppIcon, IconTone, render_icon},
        shell::floating_overlay,
    },
};

const PROMPT_VARIABLES: [&str; 3] = ["{{text}}", "{{sourceLanguage}}", "{{targetLanguage}}"];

pub(crate) fn render(
    app: &OneChat,
    kind: TranslationPromptKind,
    progress: f32,
    reduce_motion: bool,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let input = match kind {
        TranslationPromptKind::System => app.translation.controls.system_prompt.clone(),
        TranslationPromptKind::User => app.translation.controls.user_prompt.clone(),
    };
    let focus = input.read(cx).focus_handle(cx);
    let header_actions = div()
        .flex()
        .items_center()
        .gap_1()
        .child(
            Button::new("save-translation-prompt-overlay")
                .primary()
                .tooltip("Save")
                .size(px(36.0))
                .p_0()
                .rounded(px(11.0))
                .child(render_icon(AppIcon::Save, IconTone::OnAccent, 18.0, cx))
                .on_click(cx.listener(|this, _, _, cx| this.close_shell_overlay(true, cx))),
        )
        .child(
            Button::new("close-translation-prompt-overlay")
                .ghost()
                .tooltip("Close")
                .size(px(36.0))
                .p_0()
                .rounded(px(11.0))
                .child(Icon::new(IconName::Close).size(px(18.0)))
                .on_click(cx.listener(|this, _, _, cx| this.close_shell_overlay(true, cx))),
        );
    let variables = div()
        .flex()
        .flex_wrap()
        .gap_2()
        .children(PROMPT_VARIABLES.into_iter().map(|variable| {
            let input = input.clone();
            Button::new(format!(
                "insert-translation-prompt-variable-{kind:?}-{variable}"
            ))
            .ghost()
            .h(px(30.0))
            .px_3()
            .rounded(px(9.0))
            .child(variable)
            .on_click(move |_, window, cx| {
                input.update(cx, |input, cx| {
                    input.insert(variable.to_string(), window, cx);
                });
                window.focus(&input.read(cx).focus_handle(cx), cx);
            })
        }));
    let editor_height = (f32::from(window.bounds().size.height) - 250.0).clamp(280.0, 540.0);
    let editor = div()
        .h(px(editor_height))
        .min_h_0()
        .rounded(px(14.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .overflow_hidden()
        .child(
            Textarea::new(&input)
                .appearance(false)
                .size_full()
                .text_size(px(14.0))
                .line_height(px(21.0))
                .aria_label(kind.title()),
        );
    let hint = div()
        .text_size(px(11.0))
        .text_color(cx.theme().muted_foreground)
        .child("Changes apply temporarily. Save them as defaults from the translation page.");

    let panel = floating_overlay::panel(
        "translation-prompt-overlay-panel",
        kind.title(),
        &focus,
        820.0,
        22.0,
        cx,
    )
    .gap_3()
    .child(floating_overlay::header(
        kind.title(),
        "Edit the temporary prompt used by the translation workspace.",
        header_actions,
        cx,
    ))
    .child(variables)
    .child(editor)
    .child(hint);

    floating_overlay::backdrop(
        "translation-prompt-overlay",
        panel,
        progress,
        reduce_motion,
        cx,
    )
    .on_mouse_down(
        MouseButton::Left,
        cx.listener(|this, _, _, cx| this.close_shell_overlay(true, cx)),
    )
    .into_any_element()
}
