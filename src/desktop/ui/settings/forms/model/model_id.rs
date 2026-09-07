use super::*;

pub(super) fn model_id_reasoning_presets(
    editor: &ModelReasoningEditor,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let config = editor
        .suffix
        .as_ref()
        .expect("suffix mode requires discovered presets");
    let has_base = config.presets.iter().any(|preset| preset.level.is_none());
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div().text_xs().text_color(cx.theme().muted_foreground)
                .child(if has_base {
                    "Discovered from model IDs. Each preset sends the model ID shown below, without extra reasoning parameters. Default uses the base model, not Off."
                } else {
                    "Discovered from model IDs. No base model was returned, so a reasoning preset is required. Each preset sends the model ID shown below."
                })
        )
        .children(config.presets.iter().map(|preset| {
            let id = preset.id().to_string();
            div()
                .rounded(px(10.0))
                .bg(cx.theme().muted)
                .px_3()
                .py_2()
                .flex()
                .items_center()
                .gap_3()
                .child(div().w(px(72.0)).flex_none().text_sm().child(preset.label()))
                .child(div().min_w_0().flex_1().truncate().text_sm()
                    .text_color(cx.theme().muted_foreground).child(preset.model_id.clone()))
                .child(default_reasoning_action(
                    SharedString::from(format!("suffix-reasoning-default-{id}")),
                    config.default_preset == id,
                    cx,
                ).on_click(cx.listener(move |this, _, _, cx| {
                    this.set_suffix_reasoning_default(id.clone(), cx)
                })))
        }))
        .into_any_element()
}
