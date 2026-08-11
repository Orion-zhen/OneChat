mod mcp;
mod model;
mod provider;

use super::*;

fn form_input(state: &Entity<InputState>, label: &'static str) -> Input {
    Input::new(state).large().max_h(px(40.0)).aria_label(label)
}

fn single_input_row(input: Input, action: Button) -> Div {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap_2()
        .child(div().min_w_0().flex_1().child(input))
        .child(action)
}

fn key_value_input_row(name: Input, value: Input, action: Button) -> Div {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap_2()
        .child(div().min_w_0().flex_1().child(name))
        .child(div().min_w_0().flex_1().child(value))
        .child(action)
}

fn editor_header(title: &'static str, actions: impl IntoElement) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(actions)
}

pub(super) use mcp::mcp_server_form;
pub(super) use model::model_form;
pub(super) use provider::{provider_form, provider_form_actions};
