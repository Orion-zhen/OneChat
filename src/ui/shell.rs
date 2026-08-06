use std::{sync::Arc, time::Duration};

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Context, Div, ElementId, FontWeight, Image,
    ImageFormat, KeyBinding, Rgba, SharedString, Stateful, Window, WindowAppearance, actions, div,
    ease_out_quint, img, prelude::*, px, rgb, rgba,
};

use crate::{
    app::{ConnectionTestStatus, DestructiveAction, OneChat, PaletteCommand, PendingFocus},
    model::{Conversation, Page, Theme},
    ui::{chat, inspector, settings},
};

actions!(
    onechat,
    [
        NewConversation,
        ShowCommandPalette,
        ShowModelPicker,
        ToggleSidebar,
        OpenSettings,
        DismissOverlay,
    ]
);

pub fn init(cx: &mut App) {
    let primary = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    let shortcut = |key: &str| format!("{primary}-{key}");
    cx.bind_keys([
        KeyBinding::new(&shortcut("n"), NewConversation, Some("OneChat")),
        KeyBinding::new(&shortcut("k"), ShowCommandPalette, Some("OneChat")),
        KeyBinding::new(&shortcut("l"), ShowModelPicker, Some("OneChat")),
        KeyBinding::new(&shortcut("shift-s"), ToggleSidebar, Some("OneChat")),
        KeyBinding::new(&shortcut(","), OpenSettings, Some("OneChat")),
        KeyBinding::new("escape", DismissOverlay, Some("OneChat")),
    ]);
}

fn shortcut_label(key: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("⌘{key}")
    } else {
        format!("Ctrl+{key}")
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Colors {
    pub(crate) canvas: Rgba,
    pub(crate) sidebar: Rgba,
    pub(crate) toolbar: Rgba,
    pub(crate) panel: Rgba,
    pub(crate) raised: Rgba,
    pub(crate) hover: Rgba,
    pub(crate) text: Rgba,
    pub(crate) muted: Rgba,
    pub(crate) border: Rgba,
    pub(crate) accent: Rgba,
    pub(crate) accent_soft: Rgba,
    pub(crate) on_accent: Rgba,
    pub(crate) danger: Rgba,
    pub(crate) success: Rgba,
    pub(crate) scrim: Rgba,
    pub(crate) dark: bool,
}

impl Colors {
    fn for_theme(theme: Theme, appearance: WindowAppearance) -> Self {
        let dark = theme == Theme::Dark
            || (theme == Theme::System
                && matches!(
                    appearance,
                    WindowAppearance::Dark | WindowAppearance::VibrantDark
                ));
        if dark {
            Self {
                canvas: rgba(0x161618f2),
                sidebar: rgba(0x252528e8),
                toolbar: rgba(0x1d1d1fe8),
                panel: rgba(0x2c2c2ef2),
                raised: rgba(0xffffff12),
                hover: rgba(0xffffff1c),
                text: rgb(0xf5f5f7),
                muted: rgb(0xa1a1aa),
                border: rgba(0xffffff18),
                accent: rgb(0x0a84ff),
                accent_soft: rgba(0x0a84ff2e),
                on_accent: rgb(0xffffff),
                danger: rgb(0xff453a),
                success: rgb(0x30d158),
                scrim: rgba(0x00000070),
                dark: true,
            }
        } else {
            Self {
                canvas: rgba(0xf7f7f9f2),
                sidebar: rgba(0xeeeef2e8),
                toolbar: rgba(0xfafafbea),
                panel: rgba(0xfffffff2),
                raised: rgba(0x76768014),
                hover: rgba(0x76768020),
                text: rgb(0x1d1d1f),
                muted: rgb(0x6e6e73),
                border: rgba(0x3c3c431f),
                accent: rgb(0x007aff),
                accent_soft: rgba(0x007aff1f),
                on_accent: rgb(0xffffff),
                danger: rgb(0xd70015),
                success: rgb(0x248a3d),
                scrim: rgba(0x00000052),
                dark: false,
            }
        }
    }
}

pub fn render(app: &mut OneChat, window: &mut Window, cx: &mut Context<OneChat>) -> AnyElement {
    if let Some(pending) = app.pending_focus.take() {
        if pending == PendingFocus::Root {
            window.focus(&app.root_focus);
        }
        let input = match pending {
            PendingFocus::Root => None,
            PendingFocus::CommandPalette => Some(app.command_input.clone()),
            PendingFocus::ModelPicker => Some(app.model_search_input.clone()),
            PendingFocus::ConversationSearch => Some(app.search_input.clone()),
            PendingFocus::SystemPrompt => app.system_prompt_editor.clone(),
            PendingFocus::DefaultSystemPrompt => app.default_system_prompt_editor.clone(),
            PendingFocus::MessageEditor => app.active_message_editor(),
            PendingFocus::Composer if app.page == Page::Chat => Some(app.composer.clone()),
            PendingFocus::Composer => None,
        };
        if let Some(input) = input {
            window.focus(&input.read(cx).focus_handle(cx));
        }
    }

    let colors = Colors::for_theme(app.theme(), window.appearance());
    let scale_factor = window.scale_factor();
    let narrow = window.viewport_size().width < px(1120.0);
    let sidebar = (app.page == Page::Chat).then(|| render_sidebar(app, colors, scale_factor, cx));
    let top_bar = render_top_bar(app, colors, cx);
    let page = match app.page {
        Page::Chat => render_chat_page(app, colors, scale_factor, cx),
        Page::Settings => settings::render(app, colors, scale_factor, cx),
    };
    let inspector = (app.page == Page::Chat && app.inspector_open)
        .then(|| inspector::render(app, colors, narrow, cx));
    let command_palette = app
        .command_palette_open
        .then(|| render_command_palette(app, colors, cx));
    let model_picker = app
        .model_picker_open
        .then(|| render_model_picker(app, colors, cx));
    let destructive_confirmation = app
        .destructive_action
        .as_ref()
        .map(|action| render_destructive_confirmation(action, colors, cx));
    let error = app.error.clone().map(|message| {
        div()
            .absolute()
            .top(px(66.0))
            .left(px(16.0))
            .right(px(16.0))
            .rounded_xl()
            .border_1()
            .border_color(if colors.dark {
                rgba(0xff453a66)
            } else {
                rgba(0xd7001538)
            })
            .bg(if colors.dark {
                rgba(0x3a2020f5)
            } else {
                rgba(0xfff2f2f5)
            })
            .shadow_lg()
            .px_4()
            .py_3()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .text_sm()
            .text_color(colors.danger)
            .child(message)
            .child(
                compact_button("dismiss-error", "Dismiss", colors)
                    .text_color(colors.danger)
                    .on_click(cx.listener(|this, _, _, cx| this.dismiss_error(cx))),
            )
    });

    div()
        .relative()
        .size_full()
        .flex()
        .track_focus(&app.root_focus)
        .key_context("OneChat")
        .on_action(cx.listener(|this, _: &NewConversation, _, cx| {
            this.command_palette_open = false;
            this.model_picker_open = false;
            this.create_conversation(cx);
        }))
        .on_action(cx.listener(|this, _: &ShowCommandPalette, _, cx| this.open_command_palette(cx)))
        .on_action(cx.listener(|this, _: &ShowModelPicker, _, cx| this.open_model_picker(cx)))
        .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| this.toggle_sidebar(cx)))
        .on_action(cx.listener(|this, _: &OpenSettings, _, cx| this.set_page(Page::Settings, cx)))
        .on_action(cx.listener(|this, _: &DismissOverlay, _, cx| this.dismiss_overlay(cx)))
        .bg(colors.canvas)
        .text_color(colors.text)
        .text_size(px(15.0))
        .children(sidebar)
        .child(
            div()
                .relative()
                .min_w_0()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .child(top_bar)
                .child(
                    div()
                        .relative()
                        .min_h_0()
                        .flex_1()
                        .flex()
                        .child(page)
                        .children(inspector),
                )
                .children(error),
        )
        .children(command_palette)
        .children(model_picker)
        .children(destructive_confirmation)
        .into_any_element()
}

fn render_sidebar(
    app: &mut OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    if app.settings().sidebar_collapsed {
        let sidebar = div()
            .w(px(68.0))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .border_r_1()
            .border_color(colors.border)
            .bg(colors.sidebar)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .pt_3()
                    .child(
                        large_svg_icon_button(
                            "expand-sidebar",
                            UiIcon::Menu,
                            IconTone::Muted,
                            colors,
                            scale_factor,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                    )
                    .child(
                        primary_svg_icon_button(
                            "new-conversation-collapsed",
                            UiIcon::Plus,
                            colors,
                            scale_factor,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.create_conversation(cx))),
                    ),
            )
            .child(div().flex_1())
            .child(
                large_svg_icon_button(
                    "settings-collapsed",
                    UiIcon::Settings,
                    IconTone::Muted,
                    colors,
                    scale_factor,
                )
                .mb_3()
                .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Settings, cx))),
            );
        return animate_sidebar(sidebar, true);
    }

    let groups = app.conversation_groups();
    let current_id = app
        .settings()
        .current_conversation_id
        .as_deref()
        .map(str::to_owned);
    let mut list = div()
        .id("conversation-list")
        .min_h_0()
        .flex_1()
        .overflow_y_scroll()
        .px_2()
        .pb_3();
    if groups.is_empty() {
        list = list.child(
            div()
                .px_3()
                .py_5()
                .text_sm()
                .text_color(colors.muted)
                .child(if app.search_query.trim().is_empty() {
                    "No conversations yet"
                } else {
                    "No matching conversations"
                }),
        );
    } else {
        for (group, conversations) in groups {
            list = list.child(
                div()
                    .pt_4()
                    .pb_2()
                    .px_2()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.muted)
                    .child(group.label().to_uppercase()),
            );
            for conversation in conversations {
                list = list.child(render_conversation_row(
                    app,
                    conversation,
                    current_id.as_deref(),
                    colors,
                    scale_factor,
                    cx,
                ));
            }
        }
    }

    let sidebar = div()
        .w(px(260.0))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(colors.border)
        .bg(colors.sidebar)
        .child(
            div()
                .px_3()
                .pt_3()
                .pb_3()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .h(px(32.0))
                        .px_1()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(colors.muted)
                                .child("Conversations"),
                        )
                        .child(
                            large_svg_icon_button(
                                "collapse-sidebar",
                                UiIcon::ChevronLeft,
                                IconTone::Muted,
                                colors,
                                scale_factor,
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                        ),
                )
                .child(
                    primary_button_base("new-conversation", colors)
                        .w_full()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(svg_icon(
                            UiIcon::Plus,
                            IconTone::OnAccent,
                            colors,
                            scale_factor,
                            16.0,
                        ))
                        .child("New Conversation")
                        .on_click(cx.listener(|this, _, _, cx| this.create_conversation(cx))),
                )
                .child(app.search_input.clone()),
        )
        .child(list)
        .child(render_connection_footer(app, colors, scale_factor, cx));
    animate_sidebar(sidebar, false)
}

fn animate_sidebar(sidebar: Div, collapsed: bool) -> AnyElement {
    let id = if collapsed {
        "sidebar-collapsed"
    } else {
        "sidebar-expanded"
    };
    sidebar
        .overflow_hidden()
        .with_animation(
            id,
            Animation::new(Duration::from_millis(220)).with_easing(ease_out_quint()),
            move |sidebar, delta| {
                let width = if collapsed {
                    260.0 - 192.0 * delta
                } else {
                    68.0 + 192.0 * delta
                };
                sidebar.opacity(0.8 + delta * 0.2).w(px(width))
            },
        )
        .into_any_element()
}

fn render_conversation_row(
    app: &OneChat,
    conversation: Conversation,
    current_id: Option<&str>,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    if let Some(input) = app.rename_input(&conversation.id) {
        return div()
            .mb_1()
            .rounded_lg()
            .bg(colors.raised)
            .p_1()
            .child(input)
            .into_any_element();
    }

    let selected = current_id == Some(conversation.id.as_str());
    let select_id = conversation.id.clone();
    let pin_id = conversation.id.clone();
    let rename_id = conversation.id.clone();
    let delete_id = conversation.id.clone();
    let row_id: SharedString = format!("conversation-{}", conversation.id).into();
    let group_id: SharedString = format!("conversation-actions-{}", conversation.id).into();
    let pinned = conversation.pinned;

    div()
        .id(row_id)
        .group(group_id.clone())
        .mb_1()
        .h(px(38.0))
        .rounded_lg()
        .bg(if selected {
            colors.accent_soft
        } else {
            rgba(0x00000000)
        })
        .hover(move |style| {
            style.bg(if selected {
                colors.accent_soft
            } else {
                colors.hover
            })
        })
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .child(
            div()
                .id(SharedString::from(format!("select-{}", conversation.id)))
                .min_w_0()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .cursor_pointer()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_sm()
                .font_weight(if selected {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_conversation(select_id.clone(), cx)
                }))
                .child(conversation.title),
        )
        .child(
            svg_icon_button(
                SharedString::from(format!("pin-{}", pin_id)),
                UiIcon::Pin,
                if pinned {
                    IconTone::Accent
                } else {
                    IconTone::Muted
                },
                colors,
                scale_factor,
            )
            .opacity(if pinned { 1.0 } else { 0.0 })
            .group_hover(group_id.clone(), |style| style.opacity(1.0))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_pin(pin_id.clone(), cx))),
        )
        .child(
            svg_icon_button(
                SharedString::from(format!("rename-{}", rename_id)),
                UiIcon::Pencil,
                IconTone::Muted,
                colors,
                scale_factor,
            )
            .opacity(0.0)
            .group_hover(group_id.clone(), |style| style.opacity(1.0))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.start_rename(rename_id.clone(), window, cx)
            })),
        )
        .child(
            svg_icon_button(
                SharedString::from(format!("delete-{}", delete_id)),
                UiIcon::Close,
                IconTone::Danger,
                colors,
                scale_factor,
            )
            .opacity(0.0)
            .group_hover(group_id, |style| style.opacity(1.0))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.request_delete_conversation(delete_id.clone(), cx)
            })),
        )
        .into_any_element()
}

fn render_connection_footer(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let enabled = app
        .snapshot
        .providers
        .iter()
        .filter(|provider| provider.enabled)
        .count();
    div()
        .flex_none()
        .border_t_1()
        .border_color(colors.border)
        .p_3()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Settings"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(colors.muted)
                        .child(format!("{enabled} providers enabled")),
                ),
        )
        .child(
            large_svg_icon_button(
                "open-settings",
                UiIcon::Settings,
                IconTone::Muted,
                colors,
                scale_factor,
            )
            .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Settings, cx))),
        )
        .into_any_element()
}

fn render_top_bar(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    if app.page == Page::Settings {
        return div()
            .h(px(56.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px_5()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.toolbar)
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Settings"),
            )
            .child(
                compact_button("chat-page", "Done", colors)
                    .text_color(colors.accent)
                    .font_weight(FontWeight::SEMIBOLD)
                    .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Chat, cx))),
            )
            .into_any_element();
    }

    let title = app
        .current_conversation()
        .map(|conversation| conversation.title.clone())
        .unwrap_or_else(|| "OneChat".into());
    let model = app
        .current_model()
        .map(|model| model.display_name.clone())
        .unwrap_or_else(|| "Choose Model".into());
    let provider = app.current_provider();
    let provider_name = provider
        .map(|provider| provider.name.clone())
        .unwrap_or_else(|| "No provider".into());
    let (connection, connection_color) =
        provider.map_or(("Not configured", colors.muted), |provider| {
            match app.connection_tests.get(&provider.id) {
                Some(ConnectionTestStatus::Testing) => ("Testing", colors.accent),
                Some(ConnectionTestStatus::Connected) => ("Connected", colors.success),
                Some(ConnectionTestStatus::Failed(_)) => ("Connection failed", colors.danger),
                None if provider.enabled => ("Ready", colors.success),
                None => ("Disabled", colors.danger),
            }
        });

    div()
        .h(px(56.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .px_5()
        .border_b_1()
        .border_color(colors.border)
        .bg(colors.toolbar)
        .child(
            div()
                .min_w_0()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .justify_center()
                .child(
                    div()
                        .max_w(px(340.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_size(px(11.0))
                        .text_color(colors.muted)
                        .child(div().size(px(6.0)).rounded_full().bg(connection_color))
                        .child(format!("{provider_name} · {connection}")),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    button("open-model-picker", format!("{model}  ▾"), colors)
                        .max_w(px(240.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .on_click(cx.listener(|this, _, _, cx| this.open_model_picker(cx))),
                )
                .child(
                    compact_button("open-command-palette", shortcut_label("K"), colors)
                        .text_color(colors.muted)
                        .on_click(cx.listener(|this, _, _, cx| this.open_command_palette(cx))),
                )
                .child(
                    icon_button("toggle-inspector", "ⓘ", colors)
                        .when(app.inspector_open, |element| {
                            element.bg(colors.accent_soft).text_color(colors.accent)
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_inspector(cx))),
                ),
        )
        .into_any_element()
}

fn render_chat_page(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let content = if app.loading {
        empty_state(
            "Loading your conversations",
            "Opening the local OneChat library…",
            None,
            colors,
            cx,
        )
    } else if app.snapshot.providers.is_empty() {
        empty_state(
            "Connect a provider",
            "Add OpenAI, Anthropic, Gemini, or an OpenAI-compatible provider to get started.",
            Some(("Open Settings", Page::Settings)),
            colors,
            cx,
        )
    } else if !app
        .snapshot
        .models
        .iter()
        .any(|model| app.model_availability(model).is_ok())
    {
        empty_state(
            "Add your first model",
            "Choose a remote model ID for one of your configured providers.",
            Some(("Manage Models", Page::Settings)),
            colors,
            cx,
        )
    } else if app.snapshot.conversations.is_empty() {
        return empty_state(
            "What would you like to explore?",
            "Conversations and credentials stay on this Mac.",
            Some(("New Conversation", Page::Chat)),
            colors,
            cx,
        );
    } else if app.current_conversation().is_none() {
        empty_state(
            "Choose a conversation",
            "Select one from the sidebar or start a new conversation.",
            None,
            colors,
            cx,
        )
    } else {
        chat::render(app, colors, scale_factor, cx)
    };

    div()
        .min_w_0()
        .flex_1()
        .h_full()
        .child(content)
        .into_any_element()
}

fn empty_state(
    title: &'static str,
    detail: &'static str,
    action: Option<(&'static str, Page)>,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let action = action.map(|(label, page)| {
        if label == "New Conversation" {
            primary_button("empty-new-conversation", label, colors)
                .on_click(cx.listener(|this, _, _, cx| this.create_conversation(cx)))
        } else {
            primary_button("empty-state-action", label, colors)
                .on_click(cx.listener(move |this, _, _, cx| this.set_page(page, cx)))
        }
    });
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p_8()
        .child(
            div()
                .max_w(px(500.0))
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .text_center()
                .child(
                    div()
                        .size(px(52.0))
                        .rounded_full()
                        .bg(colors.accent_soft)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xl()
                        .text_color(colors.accent)
                        .child("✦"),
                )
                .child(
                    div()
                        .pt_2()
                        .text_size(px(24.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .max_w(px(440.0))
                        .line_height(px(22.0))
                        .text_color(colors.muted)
                        .child(detail),
                )
                .children(action),
        )
        .into_any_element()
}

fn render_command_palette(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let commands = app.filtered_commands();
    let mut rows = div()
        .id("command-palette-list")
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&app.command_scroll)
        .flex()
        .flex_col()
        .gap_1();
    if commands.is_empty() {
        rows = rows.child(
            div()
                .p_5()
                .text_sm()
                .text_color(colors.muted)
                .text_center()
                .child("No matching commands"),
        );
    } else {
        for (index, command) in commands.into_iter().enumerate() {
            let shortcut = command_shortcut(command);
            let selected = index == app.command_selection;
            rows = rows.child(
                div()
                    .id(SharedString::from(format!("command-{command:?}")))
                    .rounded_lg()
                    .bg(if selected {
                        colors.accent_soft
                    } else {
                        rgba(0x00000000)
                    })
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .hover(move |style| {
                        style.bg(if selected {
                            colors.accent_soft
                        } else {
                            colors.hover
                        })
                    })
                    .active(move |style| style.bg(colors.accent_soft))
                    .on_click(cx.listener(move |this, _, _, cx| this.execute_command(command, cx)))
                    .child(
                        div()
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
                                            .child(command.label()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(colors.muted)
                                            .child(command.detail()),
                                    ),
                            )
                            .children(shortcut.map(|shortcut| {
                                div()
                                    .flex_none()
                                    .rounded_md()
                                    .bg(colors.raised)
                                    .px_2()
                                    .py_1()
                                    .text_size(px(11.0))
                                    .text_color(colors.muted)
                                    .child(shortcut)
                            })),
                    ),
            );
        }
    }

    let panel = div()
        .w_full()
        .max_w(px(600.0))
        .max_h(px(560.0))
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .shadow_lg()
        .p_2()
        .flex()
        .flex_col()
        .gap_2()
        .child(app.command_input.clone())
        .child(rows)
        .child(
            div()
                .px_2()
                .pb_1()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(11.0))
                .text_color(colors.muted)
                .child("↑↓ Navigate   ↩ Select")
                .child("Esc Close"),
        );
    animated_overlay(
        panel,
        colors,
        "command-palette-backdrop",
        "command-palette-panel",
    )
}

fn command_shortcut(command: PaletteCommand) -> Option<String> {
    match command {
        PaletteCommand::NewConversation => Some(shortcut_label("N")),
        PaletteCommand::ChooseModel => Some(shortcut_label("L")),
        PaletteCommand::ToggleSidebar => Some(if cfg!(target_os = "macos") {
            "⇧⌘S".into()
        } else {
            "Ctrl+Shift+S".into()
        }),
        PaletteCommand::OpenSettings => Some(shortcut_label(",")),
        _ => None,
    }
}

fn render_model_picker(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let current_model_id = app
        .current_conversation()
        .and_then(|conversation| conversation.model_id.as_deref());
    let filtered_models = app.filtered_models();
    let mut models = div()
        .id("model-picker-list")
        .min_h_0()
        .flex_1()
        .overflow_y_scroll()
        .track_scroll(&app.model_scroll)
        .flex()
        .flex_col()
        .gap_1();

    if app.current_conversation().is_none() {
        models = models.child(notice_row(
            "Select a conversation before choosing a model.",
            colors,
        ));
    } else if app.snapshot.models.is_empty() {
        models = models.child(notice_row("No models configured.", colors));
    } else if filtered_models.is_empty() {
        models = models.child(notice_row("No models match this search.", colors));
    } else {
        for (index, model) in filtered_models.into_iter().enumerate() {
            let provider = app
                .provider_for_model(model)
                .map(|provider| provider.name.as_str())
                .unwrap_or("Missing provider");
            let availability = app.model_availability(model);
            let available = availability.is_ok();
            let status = availability.map_or_else(|reason| reason, |_| "Available");
            let current = current_model_id == Some(model.id.as_str());
            let highlighted = index == app.model_selection;
            let model_id = model.id.clone();
            models = models.child(
                div()
                    .id(SharedString::from(format!("pick-model-{}", model.id)))
                    .rounded_lg()
                    .bg(if highlighted || current {
                        colors.accent_soft
                    } else {
                        rgba(0x00000000)
                    })
                    .px_3()
                    .py_3()
                    .when(available, |element| {
                        element
                            .cursor_pointer()
                            .hover(move |style| {
                                style.bg(if highlighted || current {
                                    colors.accent_soft
                                } else {
                                    colors.hover
                                })
                            })
                            .active(move |style| style.bg(colors.accent_soft))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_model(model_id.clone(), cx)
                            }))
                    })
                    .when(!available, |element| element.opacity(0.55))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .child(
                                div()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(model.display_name.clone()),
                                    )
                                    .child(
                                        div()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .text_size(px(11.0))
                                            .text_color(colors.muted)
                                            .child(format!("{} · {provider}", model.remote_id)),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(colors.muted)
                                            .child(inspector::capability_summary(model)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_sm()
                                    .text_color(if current {
                                        colors.accent
                                    } else if available {
                                        colors.muted
                                    } else {
                                        colors.danger
                                    })
                                    .child(if current { "✓" } else { status }),
                            ),
                    ),
            );
        }
    }

    let panel = div()
        .w_full()
        .max_w(px(560.0))
        .max_h(px(640.0))
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .shadow_lg()
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .px_1()
                .pt_1()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Choose Model"),
                )
                .child(
                    icon_button("close-model-picker", "×", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.close_model_picker(cx))),
                ),
        )
        .child(app.model_search_input.clone())
        .child(models)
        .child(
            div()
                .px_1()
                .text_size(px(11.0))
                .text_color(colors.muted)
                .child("↑↓ Navigate   ↩ Select   Esc Close"),
        );
    animated_overlay(panel, colors, "model-picker-backdrop", "model-picker-panel")
}

fn notice_row(message: &str, colors: Colors) -> AnyElement {
    div()
        .rounded_lg()
        .bg(colors.raised)
        .p_4()
        .text_sm()
        .text_color(colors.muted)
        .child(message.to_string())
        .into_any_element()
}

fn render_destructive_confirmation(
    action: &DestructiveAction,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let (title, detail, confirm_label) = match action {
        DestructiveAction::DeleteConversation { title, .. } => (
            "Delete Conversation?",
            format!("“{title}” and all of its messages will be removed from this Mac."),
            "Delete",
        ),
        DestructiveAction::DeleteProvider { name, .. } => (
            "Delete Provider?",
            format!("“{name}” and its configured models will be removed from this Mac."),
            "Delete Provider",
        ),
        DestructiveAction::DeleteModel { name, .. } => (
            "Delete Model?",
            format!("“{name}” will no longer be available to conversations."),
            "Delete Model",
        ),
        DestructiveAction::ClearContext { .. } => (
            "Clear Conversation?",
            "All messages and request details in this conversation will be permanently removed."
                .to_string(),
            "Clear",
        ),
    };
    let panel = div()
        .w_full()
        .max_w(px(420.0))
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .shadow_lg()
        .p_5()
        .flex()
        .flex_col()
        .items_center()
        .gap_3()
        .text_center()
        .child(
            div()
                .size(px(48.0))
                .rounded_full()
                .bg(if colors.dark {
                    rgba(0xff453a24)
                } else {
                    rgba(0xd7001518)
                })
                .flex()
                .items_center()
                .justify_center()
                .text_xl()
                .text_color(colors.danger)
                .child("!"),
        )
        .child(
            div()
                .pt_1()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .max_w(px(350.0))
                .text_sm()
                .line_height(px(21.0))
                .text_color(colors.muted)
                .child(detail),
        )
        .child(
            div()
                .w_full()
                .pt_2()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    button("cancel-destructive-action", "Cancel", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_destructive_action(cx))),
                )
                .child(
                    destructive_button("confirm-destructive-action", confirm_label, colors)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.confirm_destructive_action(cx)),
                        ),
                ),
        );
    animated_overlay(
        panel,
        colors,
        "destructive-confirmation-backdrop",
        "destructive-confirmation-panel",
    )
}

fn animated_overlay(
    panel: Div,
    colors: Colors,
    backdrop_id: &'static str,
    panel_id: &'static str,
) -> AnyElement {
    let duration = 220;
    let panel = panel
        .with_animation(
            panel_id,
            Animation::new(Duration::from_millis(duration)).with_easing(ease_out_quint()),
            |panel, delta| {
                panel
                    .opacity(0.68 + delta * 0.32)
                    .mt(px(14.0 * (1.0 - delta)))
            },
        )
        .into_any_element();

    div()
        .id(backdrop_id)
        .occlude()
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .left_0()
        .flex()
        .items_start()
        .justify_center()
        .pt(px(96.0))
        .px_5()
        .bg(colors.scrim)
        .child(panel)
        .with_animation(
            backdrop_id,
            Animation::new(Duration::from_millis(duration)).with_easing(ease_out_quint()),
            |backdrop, delta| backdrop.opacity(delta),
        )
        .into_any_element()
}

pub(crate) fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_2()
        .rounded_lg()
        .bg(colors.raised)
        .text_sm()
        .cursor_pointer()
        .hover(move |style| style.bg(colors.hover))
        .active(move |style| style.bg(colors.accent_soft))
        .child(label.into())
}

pub(crate) fn primary_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    primary_button_base(id, colors).child(label.into())
}

fn primary_button_base(id: impl Into<ElementId>, colors: Colors) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_2()
        .rounded_lg()
        .bg(colors.accent)
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors.on_accent)
        .cursor_pointer()
        .hover(move |style| {
            style.bg(if colors.dark {
                rgb(0x2693ff)
            } else {
                rgb(0x1683ff)
            })
        })
        .active(move |style| {
            style.bg(if colors.dark {
                rgb(0x0068d6)
            } else {
                rgb(0x006ee6)
            })
        })
}

fn destructive_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_2()
        .rounded_lg()
        .bg(colors.danger)
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors.on_accent)
        .cursor_pointer()
        .hover(move |style| {
            style.bg(if colors.dark {
                rgb(0xff6259)
            } else {
                rgb(0xe31b2e)
            })
        })
        .active(move |style| {
            style.bg(if colors.dark {
                rgb(0xd92f27)
            } else {
                rgb(0xb80012)
            })
        })
        .child(label.into())
}

pub(crate) fn compact_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .px_2()
        .py_1()
        .rounded_md()
        .text_size(px(12.0))
        .cursor_pointer()
        .hover(move |style| style.bg(colors.hover))
        .active(move |style| style.bg(colors.accent_soft))
        .child(label.into())
}

pub(crate) fn primary_icon_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(32.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(colors.accent)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors.on_accent)
        .cursor_pointer()
        .hover(move |style| {
            style.bg(if colors.dark {
                rgb(0x2693ff)
            } else {
                rgb(0x1683ff)
            })
        })
        .active(move |style| {
            style.bg(if colors.dark {
                rgb(0x0068d6)
            } else {
                rgb(0x006ee6)
            })
        })
        .child(label.into())
}

pub(crate) fn destructive_icon_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(32.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(colors.danger)
        .text_color(colors.on_accent)
        .cursor_pointer()
        .hover(move |style| {
            style.bg(if colors.dark {
                rgb(0xff6259)
            } else {
                rgb(0xe31b2e)
            })
        })
        .active(move |style| {
            style.bg(if colors.dark {
                rgb(0xd92f27)
            } else {
                rgb(0xb80012)
            })
        })
        .child(label.into())
}

#[derive(Clone, Copy)]
pub(crate) enum UiIcon {
    Copy,
    Pencil,
    Regenerate,
    Info,
    Pin,
    Close,
    Settings,
    Menu,
    Plus,
    ChevronLeft,
}

#[derive(Clone, Copy)]
pub(crate) enum IconTone {
    Muted,
    Accent,
    Danger,
    OnAccent,
}

pub(crate) fn svg_icon(
    icon: UiIcon,
    tone: IconTone,
    colors: Colors,
    scale_factor: f32,
    size: f32,
) -> AnyElement {
    let display_color = match (tone, colors.dark) {
        (IconTone::Muted, true) => "#a1a1aa",
        (IconTone::Muted, false) => "#6e6e73",
        (IconTone::Accent, true) => "#0a84ff",
        (IconTone::Accent, false) => "#007aff",
        (IconTone::Danger, true) => "#ff453a",
        (IconTone::Danger, false) => "#d70015",
        (IconTone::OnAccent, _) => "#ffffff",
    };
    let image = Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        svg_icon_at_size(icon, &gpui_svg_color(display_color), scale_factor, size).into_bytes(),
    ));
    img(image).size(px(size)).into_any_element()
}

pub(crate) fn svg_icon_button(
    id: impl Into<ElementId>,
    icon: UiIcon,
    tone: IconTone,
    colors: Colors,
    scale_factor: f32,
) -> Stateful<Div> {
    svg_icon_button_sized(id, icon, tone, colors, scale_factor, 24.0, 16.0)
}

fn large_svg_icon_button(
    id: impl Into<ElementId>,
    icon: UiIcon,
    tone: IconTone,
    colors: Colors,
    scale_factor: f32,
) -> Stateful<Div> {
    svg_icon_button_sized(id, icon, tone, colors, scale_factor, 32.0, 20.0)
}

fn primary_svg_icon_button(
    id: impl Into<ElementId>,
    icon: UiIcon,
    colors: Colors,
    scale_factor: f32,
) -> Stateful<Div> {
    svg_icon_button_sized(
        id,
        icon,
        IconTone::OnAccent,
        colors,
        scale_factor,
        32.0,
        20.0,
    )
    .rounded_full()
    .bg(colors.accent)
}

fn svg_icon_button_sized(
    id: impl Into<ElementId>,
    icon: UiIcon,
    tone: IconTone,
    colors: Colors,
    scale_factor: f32,
    button_size: f32,
    icon_size: f32,
) -> Stateful<Div> {
    let (display_color, hover, active) = match (tone, colors.dark) {
        (IconTone::Muted, true) => ("#a1a1aa", colors.hover, colors.accent_soft),
        (IconTone::Muted, false) => ("#6e6e73", colors.hover, colors.accent_soft),
        (IconTone::Accent, true) => ("#0a84ff", colors.hover, colors.accent_soft),
        (IconTone::Accent, false) => ("#007aff", colors.hover, colors.accent_soft),
        (IconTone::Danger, true) => ("#ff453a", rgba(0xff453a18), rgba(0xff453a2e)),
        (IconTone::Danger, false) => ("#d70015", rgba(0xd7001512), rgba(0xd7001524)),
        (IconTone::OnAccent, true) => ("#ffffff", rgb(0x2693ff), rgb(0x0068d6)),
        (IconTone::OnAccent, false) => ("#ffffff", rgb(0x1683ff), rgb(0x006ee6)),
    };
    let svg_color = gpui_svg_color(display_color);
    let image = Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        svg_icon_at_size(icon, &svg_color, scale_factor, icon_size).into_bytes(),
    ));
    div()
        .id(id)
        .size(px(button_size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .active(move |style| style.bg(active))
        .child(img(image).size(px(icon_size)))
}

fn gpui_svg_color(display_color: &str) -> String {
    // GPUI 0.2.2 uploads SVG RGBA pixels as BGRA, so compensate before rasterization.
    format!(
        "#{}{}{}",
        &display_color[5..7],
        &display_color[3..5],
        &display_color[1..3]
    )
}

fn svg_icon_at_size(icon: UiIcon, color: &str, scale_factor: f32, size: f32) -> String {
    let paths = match icon {
        UiIcon::Copy => {
            r#"<rect width="13" height="13" x="9" y="9" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>"#
        }
        UiIcon::Pencil => {
            r#"<path d="M12 20h9"/><path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L8 18l-4 1 1-4Z"/><path d="m15 5 3 3"/>"#
        }
        UiIcon::Regenerate => {
            r#"<path d="M20 11a8.1 8.1 0 0 0-14.5-4.9L3 9"/><path d="M3 4v5h5"/><path d="M4 13a8.1 8.1 0 0 0 14.5 4.9L21 15"/><path d="M16 15h5v5"/>"#
        }
        UiIcon::Info => {
            r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v5"/><path d="M12 8h.01"/>"#
        }
        UiIcon::Pin => {
            r#"<path d="M12 17v5"/><path d="M5 17h14"/><path d="M6 17h12l-1-5 2-2V8H5v2l2 2Z"/><path d="M9 8V2h6v6"/>"#
        }
        UiIcon::Close => r#"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"#,
        UiIcon::Settings => {
            r#"<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.09a2 2 0 0 1 1 1.74v.5a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.38a2 2 0 0 0-.73-2.73l-.15-.09a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2Z"/><circle cx="12" cy="12" r="3"/>"#
        }
        UiIcon::Menu => r#"<path d="M3 6h18"/><path d="M3 12h18"/><path d="M3 18h18"/>"#,
        UiIcon::Plus => r#"<path d="M12 3v18"/><path d="M3 12h18"/>"#,
        UiIcon::ChevronLeft => r#"<path d="m16 20-8-8 8-8"/>"#,
    };
    let physical_size = (size * scale_factor.max(1.0)).round() as u32;
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{physical_size}" height="{physical_size}" viewBox="0 0 24 24" fill="none" stroke="{color}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" shape-rendering="geometricPrecision">{paths}</svg>"#
    )
}

pub(crate) fn icon_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(32.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .cursor_pointer()
        .hover(move |style| style.bg(colors.hover))
        .active(move |style| style.bg(colors.accent_soft))
        .child(label.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_icons_rasterize_at_the_display_scale() {
        let svg = svg_icon_at_size(UiIcon::Pin, "#ffffff", 2.0, 16.0);

        assert!(svg.contains(r#"width="32" height="32""#));
        assert!(svg.contains(r#"viewBox="0 0 24 24""#));
        assert!(svg.contains(r#"shape-rendering="geometricPrecision""#));

        assert_eq!(gpui_svg_color("#ff453a"), "#3a45ff");
        assert_eq!(gpui_svg_color("#0a84ff"), "#ff840a");

        let close = svg_icon_at_size(UiIcon::Close, &gpui_svg_color("#d70015"), 2.0, 16.0);
        assert!(close.contains(r##"stroke="#1500d7""##));
        assert!(close.contains(r#"<path d="M18 6 6 18"/>"#));

        let settings = svg_icon_at_size(UiIcon::Settings, "#ffffff", 2.0, 20.0);
        assert!(settings.contains(r#"width="40" height="40""#));
    }
}
