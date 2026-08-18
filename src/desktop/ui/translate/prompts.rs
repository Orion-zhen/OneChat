use gpui::{AnyElement, Context, div, prelude::*, px};
use gpui_component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
};

use crate::desktop::{
    app::{OneChat, TranslationPromptKind},
    ui::icons::{AppIcon, IconTone, render_icon},
};

pub(super) fn render(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let overridden = !app.translation.uses_default_prompts(
        &app.settings().translation_system_prompt,
        &app.settings().translation_user_prompt,
    );
    div()
        .flex_none()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_2()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child(if overridden {
                    "Using a temporary prompt override"
                } else {
                    "Using default translation prompts"
                }),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_end()
                .gap_1()
                .children(overridden.then(|| reset_button(cx)))
                .children(overridden.then(|| save_default_button(cx)))
                .child(prompt_button(
                    TranslationPromptKind::System,
                    AppIcon::Command,
                    cx,
                ))
                .child(prompt_button(
                    TranslationPromptKind::User,
                    AppIcon::MessageText,
                    cx,
                )),
        )
        .into_any_element()
}

fn reset_button(cx: &mut Context<OneChat>) -> AnyElement {
    Button::new("reset-translation-prompt-override")
        .ghost()
        .flex_none()
        .h(px(32.0))
        .px_3()
        .rounded(px(8.0))
        .child("Reset")
        .on_click(cx.listener(|this, _, _, cx| this.reset_translation_prompt_override(cx)))
        .into_any_element()
}

fn save_default_button(cx: &mut Context<OneChat>) -> AnyElement {
    Button::new("save-translation-prompts-as-default")
        .primary()
        .flex_none()
        .h(px(32.0))
        .px_3()
        .rounded(px(8.0))
        .child("Save as Default")
        .on_click(cx.listener(|this, _, _, cx| this.save_translation_prompts_as_default(cx)))
        .into_any_element()
}

fn prompt_button(
    kind: TranslationPromptKind,
    icon: AppIcon,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    Button::new(format!("open-translation-{kind:?}-prompt"))
        .ghost()
        .flex_none()
        .h(px(32.0))
        .px_3()
        .rounded(px(8.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(12.0))
                .text_color(cx.theme().muted_foreground)
                .child(render_icon(icon, IconTone::Muted, 13.0, cx))
                .child(kind.title()),
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            this.open_translation_prompt_editor(kind, window, cx)
        }))
        .into_any_element()
}
