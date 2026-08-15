use super::*;

pub(super) fn render_info(app: &OneChat, cx: &App) -> AnyElement {
    let request = app.inspected_request();
    let model = request
        .and_then(|request| request.model_id.as_deref())
        .and_then(|id| app.data.snapshot.models.iter().find(|model| model.id == id))
        .or_else(|| app.current_model());
    let provider = request
        .and_then(|request| request.provider_id.as_deref())
        .and_then(|id| {
            app.data
                .snapshot
                .providers
                .iter()
                .find(|provider| provider.id == id)
        })
        .or_else(|| app.current_provider());
    let mut content = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(inspector_field(
            "Model",
            model
                .map(|model| model.display_name.as_str())
                .unwrap_or("None"),
            cx,
        ))
        .child(inspector_field(
            "Remote ID",
            model.map(|model| model.remote_id.as_str()).unwrap_or("—"),
            cx,
        ))
        .child(inspector_field(
            "Provider",
            provider
                .map(|provider| provider.name.as_str())
                .unwrap_or("None"),
            cx,
        ));

    let Some(request) = request else {
        return content
            .child(notice("No request information yet.", cx))
            .into_any_element();
    };
    let status = match request.status {
        RequestStatus::Sending => "Sending",
        RequestStatus::Streaming => "Streaming",
        RequestStatus::Stopped => "Stopped",
        RequestStatus::Failed => "Failed",
        RequestStatus::Completed => "Completed",
        RequestStatus::Interrupted => "Interrupted",
    };
    content = content
        .child(inspector_field("Request ID", &request.id, cx))
        .child(inspector_field("Request status", status, cx))
        .child(inspector_field(
            "Input tokens",
            &format_token_count(request.usage.input_tokens, request.usage.estimated),
            cx,
        ))
        .child(inspector_field(
            "Output tokens",
            &format_token_count(request.usage.output_tokens, request.usage.estimated),
            cx,
        ))
        .child(inspector_field(
            "First token",
            &request
                .ttft_ms
                .map_or_else(|| "—".into(), |value| format!("{value} ms")),
            cx,
        ))
        .child(inspector_field(
            "Tool calls",
            &request.tool_call_count.to_string(),
            cx,
        ))
        .child(inspector_field(
            "Tool time",
            &request
                .tool_duration_ms
                .map_or_else(|| "—".into(), format_duration),
            cx,
        ))
        .child(inspector_field(
            "Total time",
            &request
                .duration_ms
                .map_or_else(|| "—".into(), |value| format!("{value} ms")),
            cx,
        ));
    if let Some(prompt) = &request.system_prompt {
        if prompt.template != prompt.resolved && !prompt.template.is_empty() {
            content = content.child(prompt_block("System prompt template", &prompt.template, cx));
        }
        content = content.child(prompt_block(
            "Resolved system prompt",
            if prompt.resolved.is_empty() {
                "—"
            } else {
                &prompt.resolved
            },
            cx,
        ));
        if !prompt.variables.is_empty() {
            let evaluations = prompt
                .variables
                .iter()
                .map(|variable| {
                    format!(
                        "{} · {} · {} ms",
                        variable.name, variable.source, variable.duration_ms
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            content = content.child(prompt_block("Prompt variables", &evaluations, cx));
        }
    }
    if let Some(opening) = &request.assistant_opening {
        if opening.template != opening.resolved && !opening.template.is_empty() {
            content = content.child(prompt_block(
                "Assistant opening template",
                &opening.template,
                cx,
            ));
        }
        content = content.child(prompt_block(
            "Resolved assistant opening",
            if opening.resolved.is_empty() {
                "—"
            } else {
                &opening.resolved
            },
            cx,
        ));
        if !opening.variables.is_empty() {
            let evaluations = opening
                .variables
                .iter()
                .map(|variable| {
                    format!(
                        "{} · {} · {} ms",
                        variable.name, variable.source, variable.duration_ms
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            content = content.child(prompt_block(
                "Assistant opening variables",
                &evaluations,
                cx,
            ));
        }
    }
    if let Some(error) = &request.error {
        content = content
            .child(inspector_field("Error category", &error.kind, cx))
            .child(
                div()
                    .rounded_lg()
                    .bg(cx.theme().muted)
                    .p_3()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.message.clone())
                    .children(error.detail.clone().map(|detail| {
                        div()
                            .pt_2()
                            .text_size(px(11.0))
                            .text_color(cx.theme().muted_foreground)
                            .child(detail)
                    })),
            );
    }
    content.into_any_element()
}

fn prompt_block(label: &str, value: &str, cx: &App) -> AnyElement {
    div()
        .rounded_lg()
        .bg(cx.theme().muted)
        .p_3()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .pt_2()
                .whitespace_normal()
                .text_size(px(12.0))
                .line_height(px(18.0))
                .child(value.to_string()),
        )
        .into_any_element()
}

fn format_duration(duration_ms: u64) -> String {
    if duration_ms < 1_000 {
        format!("{duration_ms} ms")
    } else {
        format!("{}.{:01} s", duration_ms / 1_000, duration_ms % 1_000 / 100)
    }
}

fn format_token_count(value: Option<u64>, estimated: bool) -> String {
    value.map_or_else(
        || "—".into(),
        |value| {
            if estimated {
                format!("~{value}")
            } else {
                value.to_string()
            }
        },
    )
}
