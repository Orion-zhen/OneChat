mod context;
mod editor;
mod info;
mod model;
mod tools;

use context::render_context;
use info::render_info;
use model::render_model;
use tools::render_tools;

pub(crate) use editor::{
    GenerationConfigEditor, GenerationParameter, GenerationParameterItem, ReasoningPresetItem,
};

use std::{fmt::Display, str::FromStr};

use gpui::{
    AnyElement, App, Context, Entity, FontWeight, SharedString, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Sizable as _,
    alert::Alert,
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState, MaskPattern},
    searchable_list::SearchableListItem,
    select::{Select, SelectState},
    slider::Slider,
    switch::Switch,
    tab::{Tab, TabBar},
};
use serde_json::{Map, Value};

use crate::{
    desktop::app::OneChat,
    desktop::ui::{
        badges::{StatusPillBackground, status_pill},
        controls::sync_slider,
        icons::{AppIcon, IconActionSize::Regular, IconTone, render_icon},
        input::multiline as multiline_input,
        mcp::tool_row as mcp_tool_row,
    },
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

pub(crate) fn sync_controls(app: &mut OneChat, window: &mut Window, cx: &mut Context<OneChat>) {
    let history_limit = app.displayed_history_limit().slider_value();
    sync_slider(&app.chat.history_limit_slider, history_limit, window, cx);

    app.sync_generation_config_editor(window, cx);
    let model = app.current_model().cloned();
    if let (Some(editor), Some(model)) = (&mut app.chat.generation_config_editor, model) {
        editor.sync_parameter_select(&model.capabilities, window, cx);
        editor.sync_reasoning_select(&model, window, cx);
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
        .bg(crate::desktop::ui::theme::palette(cx).overlay_panel)
        .p_4()
        .flex()
        .flex_col()
        .gap_4()
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
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
                    Regular
                        .icon_action(
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
