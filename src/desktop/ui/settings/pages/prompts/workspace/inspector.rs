use std::collections::BTreeSet;

use super::super::super::super::*;
use crate::application::prompt::referenced_prompt_variables;
use gpui_component::scroll::ScrollableElement as _;

pub(super) struct PromptMetrics {
    pub(super) characters: usize,
    pub(super) lines: usize,
    pub(super) variables: usize,
    undefined: Vec<String>,
    template_error: Option<String>,
}

pub(super) fn inspector(
    app: &OneChat,
    editor: &PromptPresetEditor,
    editing: bool,
    metrics: &PromptMetrics,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .w(px(268.0))
        .h_full()
        .flex_none()
        .border_l_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().sidebar)
        .overflow_y_scrollbar()
        .p_4()
        .flex()
        .flex_col()
        .gap_5()
        .child(inspector_section(
            "PRESET",
            Input::new(&editor.name)
                .aria_label("Preset name")
                .large()
                .rounded(px(10.0))
                .disabled(!editing),
            cx,
        ))
        .children(editing.then(|| variable_section(app, cx)))
        .child(validation_section(metrics, cx))
        .into_any_element()
}

fn inspector_section(title: &'static str, content: impl IntoElement, cx: &App) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(title),
        )
        .child(content)
        .into_any_element()
}

fn variable_section(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let custom = app
        .settings()
        .prompt_variables
        .iter()
        .map(|(name, source)| (name.clone(), source.preview().to_string()))
        .collect::<Vec<_>>();

    inspector_section(
        "VARIABLES",
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_1()
            .children(
                custom
                    .into_iter()
                    .map(|(name, description)| variable_row(name, description, false, cx)),
            )
            .children(
                BUILTIN_PROMPT_VARIABLES
                    .into_iter()
                    .map(|(name, description)| {
                        variable_row(name.to_string(), description.to_string(), true, cx)
                    }),
            ),
        cx,
    )
}

fn variable_row(
    name: String,
    description: String,
    builtin: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let id = SharedString::from(format!("insert-prompt-variable-{name}"));
    let inserted_name = name.clone();
    div()
        .id(id)
        .w_full()
        .rounded(px(8.0))
        .px_2()
        .py_2()
        .cursor_pointer()
        .hover(|style| style.bg(cx.theme().list_hover))
        .child(
            div()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(format!("{{{{{name}}}}}")),
        )
        .child(
            div()
                .pt_0p5()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child(if builtin {
                    description
                } else if description.is_empty() {
                    "Custom variable".to_string()
                } else {
                    description
                }),
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            this.insert_prompt_preset_variable(inserted_name.clone(), window, cx)
        }))
        .into_any_element()
}

fn validation_section(metrics: &PromptMetrics, cx: &App) -> AnyElement {
    let (tone, message) = if let Some(error) = &metrics.template_error {
        (IconTone::Danger, error.clone())
    } else if metrics.undefined.is_empty() {
        (IconTone::Success, "No unresolved variables".to_string())
    } else {
        (
            IconTone::Warning,
            format!("Undefined: {}", metrics.undefined.join(", ")),
        )
    };

    inspector_section(
        "VALIDATION",
        div()
            .w_full()
            .rounded(px(10.0))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .p_3()
            .flex()
            .items_start()
            .gap_2()
            .child(render_icon(
                if tone == IconTone::Success {
                    AppIcon::ContextSelected
                } else {
                    AppIcon::Info
                },
                tone,
                14.0,
                cx,
            ))
            .child(
                div()
                    .min_w_0()
                    .whitespace_normal()
                    .text_size(px(11.0))
                    .line_height(px(17.0))
                    .child(message),
            ),
        cx,
    )
}

pub(super) fn prompt_metrics(
    app: &OneChat,
    editor: &PromptPresetEditor,
    section: PromptPresetSection,
    cx: &App,
) -> PromptMetrics {
    let active_text = editor.text(section, cx);
    let templates = [
        editor.text(PromptPresetSection::SystemPrompt, cx),
        editor.text(PromptPresetSection::AssistantOpening, cx),
    ];
    let mut references = BTreeSet::new();
    let mut template_error = None;
    for template in templates {
        match referenced_prompt_variables(&template) {
            Ok(found) => references.extend(found),
            Err(error) => {
                template_error = Some(error.to_string());
                break;
            }
        }
    }
    let defined = app
        .settings()
        .prompt_variables
        .keys()
        .map(String::as_str)
        .chain(BUILTIN_PROMPT_VARIABLES.iter().map(|(name, _)| *name))
        .collect::<BTreeSet<_>>();
    let undefined = references
        .iter()
        .filter(|name| !defined.contains(name.as_str()))
        .cloned()
        .collect();

    PromptMetrics {
        characters: active_text.chars().count(),
        lines: if active_text.is_empty() {
            0
        } else {
            active_text.lines().count()
        },
        variables: references.len(),
        undefined,
        template_error,
    }
}
