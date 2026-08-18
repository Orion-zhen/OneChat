mod components;
mod presets;
mod titles;
mod translations;
mod variable_dialog;
mod variables;
mod workspace;

use super::super::*;
use presets::{default_prompt_select, prompt_presets_content};
use titles::title_prompt_content;
use translations::translation_prompts_content;
use variables::prompt_variables_content;

pub(in crate::desktop::ui::settings) use variable_dialog::prompt_variable_dialog_body;
pub(in crate::desktop::ui::settings) use workspace::prompt_preset_workspace;

pub(in crate::desktop::ui::settings) fn system_prompts_page(
    app: &OneChat,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let preset_count = app.data.snapshot.prompt_presets.len();
    let preset_count_label = format!(
        "{preset_count} {}",
        if preset_count == 1 {
            "preset"
        } else {
            "presets"
        }
    );
    let library_actions = div()
        .flex_none()
        .flex()
        .items_center()
        .gap_2()
        .child(status_pill(
            preset_count_label,
            false,
            StatusPillBackground::Muted,
            cx,
        ))
        .child(
            Compact
                .icon_action(
                    "reload-prompt-presets",
                    AppIcon::Regenerate,
                    IconTone::Muted,
                    "Reload presets",
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.reload_prompt_presets(cx))),
        )
        .child(
            Compact
                .primary_icon_action("add-prompt-preset", AppIcon::Plus, "New preset", cx)
                .on_click(
                    cx.listener(|this, _, window, cx| this.begin_add_prompt_preset(window, cx)),
                ),
        );
    let variable_actions = Compact
        .primary_icon_action("add-prompt-variable", AppIcon::Plus, "New variable", cx)
        .on_click(cx.listener(|this, _, window, cx| this.begin_add_prompt_variable(window, cx)));

    let conversation_prompts = div()
        .w_full()
        .flex()
        .flex_col()
        .child(setting_row(
            "Default Prompt Preset",
            "Copies its system prompt and optional assistant opening into each new conversation.",
            default_prompt_select(app),
            cx,
        ))
        .child(setting_divider(cx))
        .child(
            div()
                .px_4()
                .pt_4()
                .pb_2()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    div()
                        .min_w_0()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Preset Library"),
                        )
                        .child(
                            div()
                                .pt_1()
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted_foreground)
                                .child("Reusable system prompts with optional assistant openings."),
                        ),
                )
                .child(library_actions),
        )
        .child(prompt_presets_content(app, cx));

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "Prompts",
                "Create reusable conversation presets and add context at runtime.",
                cx,
            ))
            .child(section(
                "Conversation Presets",
                Some("Choose the system prompt and optional assistant opening for new conversations."),
                conversation_prompts,
                cx,
            ))
            .child(section(
                "Automatic Titles",
                Some("Instructions used after the first completed response."),
                title_prompt_content(app, cx),
                cx,
            ))
            .child(section(
                "Translation",
                Some("Defaults used by the translation workspace. Temporary overrides stay local to the current app session."),
                translation_prompts_content(app, cx),
                cx,
            ))
            .child(section_with_actions(
                "Variables",
                Some("Insert dynamic values with {{name}}."),
                Some(variable_actions.into_any_element()),
                prompt_variables_content(app, cx),
                cx,
            )),
    )
}
