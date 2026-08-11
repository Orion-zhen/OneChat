use super::*;

pub(super) fn argument_fields(editor: &McpServerEditor, cx: &mut Context<OneChat>) -> AnyElement {
    let arguments =
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .children(editor.args.iter().enumerate().map(|(index, argument)| {
                let is_draft = index + 1 == editor.args.len();
                let action = if is_draft {
                    Compact
                        .icon_action(
                            "add-mcp-argument",
                            AppIcon::Plus,
                            IconTone::Accent,
                            "Add argument",
                            cx,
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| this.add_mcp_argument(window, cx)),
                        )
                } else {
                    Compact
                        .icon_action(
                            SharedString::from(format!("remove-mcp-argument-{index}")),
                            AppIcon::Trash,
                            IconTone::Danger,
                            "Remove argument",
                            cx,
                        )
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.remove_mcp_argument(index, cx)),
                        )
                };
                single_input_row(form_input(argument, "MCP server argument"), action)
            }));
    arguments.into_any_element()
}

pub(super) fn environment_fields(
    editor: &McpServerEditor,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let environment =
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .children(editor.env.iter().enumerate().map(|(index, variable)| {
                let is_draft = index + 1 == editor.env.len();
                let action = if is_draft {
                    Compact
                        .icon_action(
                            "add-mcp-environment",
                            AppIcon::Plus,
                            IconTone::Accent,
                            "Add environment variable",
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.add_mcp_environment_variable(window, cx)
                        }))
                } else {
                    Compact
                        .icon_action(
                            SharedString::from(format!("remove-mcp-environment-{index}")),
                            AppIcon::Trash,
                            IconTone::Danger,
                            "Remove environment variable",
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove_mcp_environment_variable(index, cx)
                        }))
                };
                key_value_input_row(
                    form_input(&variable.name, "Environment variable name"),
                    form_input(&variable.value, "Environment variable value"),
                    action,
                )
            }));
    environment.into_any_element()
}

pub(super) fn header_fields(editor: &McpServerEditor, cx: &mut Context<OneChat>) -> AnyElement {
    let headers =
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .children(editor.headers.iter().enumerate().map(|(index, header)| {
                let is_draft = index + 1 == editor.headers.len();
                let action = if is_draft {
                    Compact
                        .icon_action(
                            "add-mcp-header",
                            AppIcon::Plus,
                            IconTone::Accent,
                            "Add HTTP header",
                            cx,
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| this.add_mcp_header(window, cx)),
                        )
                } else {
                    Compact
                        .icon_action(
                            SharedString::from(format!("remove-mcp-header-{index}")),
                            AppIcon::Trash,
                            IconTone::Danger,
                            "Remove HTTP header",
                            cx,
                        )
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.remove_mcp_header(index, cx)),
                        )
                };
                key_value_input_row(
                    form_input(&header.name, "HTTP header name"),
                    form_input(&header.value, "HTTP header value"),
                    action,
                )
            }));
    headers.into_any_element()
}
