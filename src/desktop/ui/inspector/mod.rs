mod editor;

pub use editor::{GenerationConfigEditor, GenerationParameter, GenerationParameterItem};

use std::{fmt::Display, str::FromStr};

use gpui::{
    AnyElement, App, Context, Entity, FontWeight, MouseButton, MouseDownEvent, MouseUpEvent,
    SharedString, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Sizable as _,
    alert::Alert,
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState, MaskPattern},
    searchable_list::SearchableListItem,
    select::{Select, SelectState},
    switch::Switch,
    tab::{Tab, TabBar},
};
use serde_json::{Map, Value};

use crate::{
    desktop::app::OneChat,
    desktop::ui::icons::{AppIcon, IconTone, render_icon},
    domain::{Conversation, GenerationConfig, Model, RequestStatus, ToolSelection},
    mcp::McpServerStatus,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InspectorTab {
    #[default]
    Model,
    Context,
    Tools,
    Info,
}

fn icon_action(
    id: impl Into<gpui::ElementId>,
    icon: AppIcon,
    tone: IconTone,
    tooltip: &'static str,
    cx: &App,
) -> Button {
    Button::new(id)
        .ghost()
        .tooltip(tooltip)
        .size(px(36.0))
        .p_0()
        .child(render_icon(icon, tone, 19.0, cx))
}

fn primary_icon_action(
    id: impl Into<gpui::ElementId>,
    icon: AppIcon,
    tooltip: &'static str,
    cx: &App,
) -> Button {
    icon_action(id, icon, IconTone::OnAccent, tooltip, cx).primary()
}

fn danger_icon_action(
    id: impl Into<gpui::ElementId>,
    icon: AppIcon,
    tooltip: &'static str,
    cx: &App,
) -> Button {
    icon_action(id, icon, IconTone::OnAccent, tooltip, cx).danger()
}

pub(crate) fn sync_controls(app: &mut OneChat, window: &mut Window, cx: &mut Context<OneChat>) {
    app.sync_generation_config_editor(window, cx);
    let capabilities = app.current_model().map(|model| model.capabilities.clone());
    if let (Some(editor), Some(capabilities)) =
        (&mut app.chat.generation_config_editor, capabilities)
    {
        editor.sync_parameter_select(&capabilities, window, cx);
    }
}

pub(crate) fn render(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let selected_tab = match app.navigation.inspector_tab {
        InspectorTab::Model => 0,
        InspectorTab::Context => 1,
        InspectorTab::Tools => 2,
        InspectorTab::Info => 3,
    };
    let tabs = TabBar::new("inspector-tabs")
        .segmented()
        .large()
        .w_full()
        .selected_index(selected_tab)
        .child(Tab::new().w(px(76.0)).label("Model"))
        .child(Tab::new().w(px(76.0)).label("Context"))
        .child(Tab::new().w(px(76.0)).label("Tools"))
        .child(Tab::new().w(px(76.0)).label("Info"))
        .on_click(cx.listener(|this, index: &usize, _, cx| {
            let tab = [
                InspectorTab::Model,
                InspectorTab::Context,
                InspectorTab::Tools,
                InspectorTab::Info,
            ][*index];
            this.set_inspector_tab(tab, cx);
        }));

    let content = match app.navigation.inspector_tab {
        InspectorTab::Model => render_model(app, cx),
        InspectorTab::Context => render_context(app, cx),
        InspectorTab::Tools => render_tools(app, cx),
        InspectorTab::Info => render_info(app, cx),
    };

    div()
        .absolute()
        .occlude()
        .top(px(8.0))
        .right(px(8.0))
        .bottom(px(16.0))
        .w(px(352.0))
        .shadow_lg()
        .rounded(px(16.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .p_4()
        .flex()
        .flex_col()
        .gap_4()
        .when(app.navigation.inspector_open, |inspector| {
            inspector
                .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, _, _| {
                    if event.button == MouseButton::Left {
                        this.cancel_inspector_outside_press();
                    }
                }))
                .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, _| {
                    if event.button == MouseButton::Left {
                        this.begin_inspector_outside_press();
                    }
                }))
                .capture_any_mouse_up(cx.listener(|this, event: &MouseUpEvent, _, _| {
                    if event.button == MouseButton::Left {
                        this.cancel_inspector_outside_press();
                    }
                }))
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, window, cx| {
                        if window.has_active_prompt() {
                            this.cancel_inspector_outside_press();
                        } else {
                            this.release_inspector_outside(cx);
                        }
                    }),
                )
        })
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Details"),
                )
                .child(
                    icon_action(
                        "close-inspector",
                        AppIcon::Close,
                        IconTone::Muted,
                        "Close details",
                        cx,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.close_inspector(cx))),
                ),
        )
        .child(tabs)
        .child(
            div()
                .id("inspector-content-scroll")
                .min_h_0()
                .flex_1()
                .overflow_y_scroll()
                .child(content),
        )
        .into_any_element()
}

fn render_model(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let Some(conversation) = app.current_conversation() else {
        return notice("Select a conversation to configure its model.", cx);
    };
    let Some(model) = app.current_model() else {
        return div()
            .flex()
            .flex_col()
            .gap_3()
            .child(notice("This conversation has no model.", cx))
            .child(
                primary_icon_action(
                    "inspector-choose-model-empty",
                    AppIcon::Layers,
                    "Choose model",
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| this.open_model_picker(window, cx))),
            )
            .into_any_element();
    };

    let provider = app
        .current_provider()
        .map(|provider| provider.name.as_str())
        .unwrap_or("Missing provider");
    let ignored = conversation
        .generation_config
        .filtered_for(&model.capabilities)
        .1;
    let Some(editor) = app.chat.generation_config_editor.as_ref() else {
        return notice("Opening parameter editor…", cx);
    };

    let mut parameters = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(model_summary(model, provider, cx));

    if !ignored.is_empty() {
        parameters = parameters.child(
            div()
                .rounded_lg()
                .bg(cx.theme().accent)
                .p_3()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                    "Not sent by this model: {}. The saved values are preserved.",
                    ignored.join(", ")
                )),
        );
    }

    let capabilities = &model.capabilities;
    for parameter in GenerationParameter::ALL {
        if editor.is_active(parameter) && parameter.supported_by(capabilities) {
            parameters = parameters.child(parameter_field(parameter, editor.input(parameter), cx));
        }
    }

    parameters
        .child(add_parameter_select(editor, capabilities, cx))
        .children(
            app.chat
                .parameter_error
                .as_ref()
                .map(|error| Alert::error("generation-parameter-error", error.clone()).small()),
        )
        .into_any_element()
}

fn tool_status_pill(label: impl Into<SharedString>, accent: bool, cx: &App) -> AnyElement {
    div()
        .flex_none()
        .rounded_full()
        .bg(if accent {
            cx.theme().accent
        } else {
            cx.theme().background
        })
        .px_2()
        .py_1()
        .text_size(px(10.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if accent {
            cx.theme().primary
        } else {
            cx.theme().muted_foreground
        })
        .child(label.into())
        .into_any_element()
}

fn render_tools(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let Some(conversation) = app.current_conversation() else {
        return notice("Select a conversation to configure its tools.", cx);
    };
    let generating = app.is_current_generating();
    let model_supports_tools = app
        .current_model()
        .is_some_and(|model| model.capabilities.tools);
    let available_count = app
        .mcp
        .snapshot
        .servers
        .iter()
        .filter(|server| server.enabled && server.status == McpServerStatus::Ready)
        .flat_map(|server| server.tools.iter())
        .count();
    let selected_count = app
        .mcp
        .snapshot
        .servers
        .iter()
        .filter(|server| server.enabled && server.status == McpServerStatus::Ready)
        .flat_map(|server| {
            server.tools.iter().filter(move |tool| {
                conversation
                    .tool_selection
                    .resolves(&server.id, &tool.name, tool.enabled)
            })
        })
        .count();
    let summary = match &conversation.tool_selection {
        ToolSelection::Default => format!(
            "{selected_count} of {available_count} available tools use the global defaults."
        ),
        ToolSelection::Only(_) => {
            format!("{selected_count} of {available_count} available tools are enabled.")
        }
    };

    let mut content =
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(cx.theme().muted_foreground)
                    .child(summary),
            )
            .when(!model_supports_tools, |content| {
                content.child(notice("The current model does not support tools.", cx))
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("enable-all-conversation-tools")
                            .small()
                            .compact()
                            .label("Enable all")
                            .disabled(generating)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_all_conversation_tools(true, cx)
                            })),
                    )
                    .child(
                        Button::new("reset-conversation-tools")
                            .small()
                            .compact()
                            .label("Use defaults")
                            .disabled(generating)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.reset_conversation_tool_selection(cx)
                            })),
                    )
                    .child(
                        Button::new("disable-all-conversation-tools")
                            .small()
                            .compact()
                            .label("Disable all")
                            .disabled(generating)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_all_conversation_tools(false, cx)
                            })),
                    ),
            );

    let mut has_tools = false;
    for server in &app.mcp.snapshot.servers {
        if server.tools.is_empty() {
            continue;
        }
        has_tools = true;
        let expanded = app
            .chat
            .expanded_conversation_tool_server_ids
            .contains(&server.id);
        let server_available = server.enabled && server.status == McpServerStatus::Ready;
        let enabled_count = server
            .tools
            .iter()
            .filter(|tool| {
                conversation
                    .tool_selection
                    .resolves(&server.id, &tool.name, tool.enabled)
            })
            .count();
        let all_enabled = server_available && enabled_count == server.tools.len();
        let tool_count = server.tools.len();
        let tool_label = format!(
            "{tool_count} {}",
            if tool_count == 1 { "tool" } else { "tools" }
        );
        let status = if server_available {
            format!("{enabled_count} enabled")
        } else {
            "Unavailable".to_string()
        };
        let toggle_id = server.id.clone();
        let expand_id = server.id.clone();

        let mut card = div()
            .w_full()
            .rounded(px(10.0))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .px_4()
            .py_3()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(server.id.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(tool_status_pill(tool_label, false, cx))
                            .child(tool_status_pill(status, all_enabled, cx))
                            .child(
                                Switch::new(SharedString::from(format!(
                                    "conversation-server-tools-{}",
                                    server.id
                                )))
                                .small()
                                .checked(all_enabled)
                                .color(cx.theme().primary)
                                .disabled(generating || !server_available || app.mcp.loading)
                                .tooltip(if all_enabled {
                                    "Disable this server's tools for the conversation"
                                } else {
                                    "Enable this server's tools for the conversation"
                                })
                                .on_click(cx.listener(
                                    move |this, enabled: &bool, _, cx| {
                                        this.set_conversation_server_tools_enabled(
                                            toggle_id.clone(),
                                            *enabled,
                                            cx,
                                        )
                                    },
                                )),
                            )
                            .child(
                                icon_action(
                                    SharedString::from(format!(
                                        "expand-conversation-tool-server-{}",
                                        server.id
                                    )),
                                    if expanded {
                                        AppIcon::ChevronUp
                                    } else {
                                        AppIcon::ChevronDown
                                    },
                                    IconTone::Muted,
                                    if expanded {
                                        "Collapse tool server"
                                    } else {
                                        "Expand tool server"
                                    },
                                    cx,
                                )
                                .size(px(24.0))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.toggle_conversation_tool_server(expand_id.clone(), cx)
                                    },
                                )),
                            ),
                    ),
            );

        if expanded {
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(server.tools.iter().map(|tool| {
                        let checked = server_available
                            && conversation.tool_selection.resolves(
                                &server.id,
                                &tool.name,
                                tool.enabled,
                            );
                        let server_id = server.id.clone();
                        let tool_name = tool.name.clone();
                        let label = tool.title.as_deref().unwrap_or(&tool.name).to_string();
                        div()
                            .rounded(px(7.0))
                            .bg(cx.theme().popover)
                            .px_3()
                            .py_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(label),
                                    )
                                    .child(
                                        Switch::new(SharedString::from(format!(
                                            "conversation-tool-{}-{}",
                                            server.id, tool.name
                                        )))
                                        .small()
                                        .checked(checked)
                                        .color(cx.theme().primary)
                                        .disabled(
                                            generating || !server_available || app.mcp.loading,
                                        )
                                        .tooltip(if server_available {
                                            "Override this tool for the conversation"
                                        } else {
                                            "The MCP server must be enabled and connected"
                                        })
                                        .on_click(
                                            cx.listener(move |this, enabled: &bool, _, cx| {
                                                this.set_conversation_tool_enabled(
                                                    server_id.clone(),
                                                    tool_name.clone(),
                                                    *enabled,
                                                    cx,
                                                )
                                            }),
                                        ),
                                    ),
                            )
                            .children(tool.description.as_ref().map(|description| {
                                div()
                                    .pt_0p5()
                                    .text_size(px(11.0))
                                    .line_height(px(16.0))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(description.clone())
                            }))
                    })),
            );
        }
        content = content.child(card);
    }

    if !has_tools {
        content = content.child(notice(
            "No MCP tools have been discovered. Configure them in Settings.",
            cx,
        ));
    }
    content.into_any_element()
}

fn render_context(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let Some(conversation) = app.current_conversation() else {
        return notice("Select a conversation to inspect its context.", cx);
    };
    let prompt = if conversation.system_prompt.trim().is_empty() {
        "None".to_string()
    } else {
        conversation.system_prompt.clone()
    };
    let source = app.system_prompt_label(&conversation.system_prompt);
    let estimated_tokens = estimate_context_tokens(app);

    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(inspector_field("System Prompt", &prompt, cx))
        .child(inspector_field("Prompt source", &source, cx))
        .child(inspector_field(
            "Messages",
            &app.current_context_messages().len().to_string(),
            cx,
        ))
        .child(inspector_field(
            "Estimated context tokens",
            &format!("~{estimated_tokens}"),
            cx,
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    primary_icon_action(
                        "context-edit-system-prompt",
                        AppIcon::Pencil,
                        "Edit system prompt",
                        cx,
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| {
                            this.begin_edit_system_prompt(window, cx)
                        }),
                    ),
                )
                .child(
                    danger_icon_action(
                        "clear-conversation-context",
                        AppIcon::Trash,
                        "Clear context",
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.request_clear_current_context(window, cx)
                    })),
                ),
        )
        .into_any_element()
}

fn render_info(app: &OneChat, cx: &App) -> AnyElement {
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

pub(crate) fn capability_summary(model: &Model) -> String {
    let capabilities = &model.capabilities;
    let mut labels = Vec::new();
    if capabilities.streaming {
        labels.push("Streaming");
    }
    if capabilities.tools {
        labels.push("Tools");
    }
    if capabilities.vision {
        labels.push("Vision");
    }
    if labels.is_empty() {
        "No declared capabilities".into()
    } else {
        labels.join(" · ")
    }
}

fn estimate_context_tokens(app: &OneChat) -> usize {
    let characters = app
        .current_conversation()
        .map(|conversation| conversation.system_prompt.chars().count())
        .unwrap_or_default()
        + app
            .current_context_messages()
            .iter()
            .map(|message| serde_json::to_string(message).map_or(0, |value| value.chars().count()))
            .sum::<usize>();
    characters.div_ceil(4)
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

fn parameter_field(
    parameter: GenerationParameter,
    input: Entity<InputState>,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let label = div()
        .flex()
        .flex_col()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(parameter.label()),
        )
        .child(
            div()
                .pt(px(2.0))
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child(parameter.hint()),
        );
    let remove = Button::new(SharedString::from(format!(
        "remove-parameter-{}",
        parameter.id()
    )))
    .ghost()
    .tooltip("Remove parameter")
    .size(px(30.0))
    .p_0()
    .child(render_icon(AppIcon::Close, IconTone::Muted, 15.0, cx))
    .on_click(cx.listener(move |this, _, window, cx| {
        this.remove_generation_parameter(parameter, window, cx)
    }));

    if parameter.is_multiline() {
        div()
            .rounded(px(14.0))
            .bg(cx.theme().muted)
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap_2()
                    .child(label)
                    .child(remove),
            )
            .child(
                Input::new(&input)
                    .aria_label(parameter.label())
                    .h(if parameter == GenerationParameter::Extra {
                        px(140.0)
                    } else {
                        px(92.0)
                    })
                    .rounded(px(10.0)),
            )
            .into_any_element()
    } else {
        div()
            .rounded(px(14.0))
            .bg(cx.theme().muted)
            .p_3()
            .flex()
            .items_center()
            .gap_2()
            .child(div().min_w_0().flex_1().child(label))
            .child(
                Input::new(&input)
                    .aria_label(parameter.label())
                    .w(px(112.0))
                    .h(px(40.0))
                    .px_3()
                    .rounded(px(10.0))
                    .text_right(),
            )
            .child(remove)
            .into_any_element()
    }
}

fn add_parameter_select(
    editor: &GenerationConfigEditor,
    capabilities: &crate::domain::ModelCapabilities,
    cx: &App,
) -> AnyElement {
    let disabled = GenerationParameter::ALL
        .into_iter()
        .all(|parameter| !parameter.supported_by(capabilities) || editor.is_active(parameter));
    Select::new(&editor.parameter_select)
        .large()
        .h(px(44.0))
        .px(px(14.0))
        .rounded(px(12.0))
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .placeholder(if disabled {
            "All available parameters added"
        } else {
            "Add parameter"
        })
        .disabled(disabled)
        .w_full()
        .into_any_element()
}

fn model_summary(model: &Model, provider: &str, cx: &App) -> AnyElement {
    div()
        .rounded_lg()
        .bg(cx.theme().muted)
        .p_3()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child("Model"),
        )
        .child(
            div()
                .pt_1()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(model.display_name.clone()),
        )
        .child(
            div()
                .pt_1()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child(format!("{} · {}", provider, capability_summary(model))),
        )
        .into_any_element()
}

fn inspector_field(label: &str, value: &str, cx: &App) -> AnyElement {
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
        .child(div().pt_1().text_sm().child(value.to_string()))
        .into_any_element()
}

fn notice(message: &str, cx: &App) -> AnyElement {
    div()
        .rounded_xl()
        .bg(cx.theme().muted)
        .p_4()
        .text_sm()
        .line_height(px(21.0))
        .text_color(cx.theme().muted_foreground)
        .child(message.to_string())
        .into_any_element()
}
