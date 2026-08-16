use std::{rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt as _, App, ElementId, IntoElement, RenderOnce, SharedString,
    StyleRefinement, Styled, Window, div, ease_in_out, prelude::*, px, relative,
};
use gpui_component::{
    ActiveTheme as _, Selectable as _, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
};

type ClickHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub(crate) struct SegmentedControl {
    id: ElementId,
    labels: Vec<SharedString>,
    selected_index: usize,
    style: StyleRefinement,
    on_click: Option<ClickHandler>,
}

impl SegmentedControl {
    pub(crate) fn new(
        id: impl Into<ElementId>,
        labels: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        let labels = labels.into_iter().map(Into::into).collect::<Vec<_>>();
        assert!(!labels.is_empty(), "segmented control requires an option");
        Self {
            id: id.into(),
            labels,
            selected_index: 0,
            style: StyleRefinement::default(),
            on_click: None,
        }
    }

    pub(crate) fn selected_index(mut self, index: usize) -> Self {
        assert!(
            index < self.labels.len(),
            "selected segment is out of range"
        );
        self.selected_index = index;
        self
    }

    pub(crate) fn on_click(
        mut self,
        handler: impl Fn(&usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Styled for SegmentedControl {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let selected_index = self.selected_index;
        let segment_count = self.labels.len() as f32;
        let on_click = self.on_click;
        let animation_key: SharedString = format!("{}-indicator", self.id).into();
        let motion = window.use_keyed_state(animation_key.clone(), cx, |_, _| {
            (selected_index, selected_index, 0_u64)
        });
        let mut motion_params = *motion.read(cx);
        if motion_params.0 != selected_index {
            motion_params = (selected_index, motion_params.0, motion_params.2 + 1);
            motion.update(cx, |motion, _| *motion = motion_params);
        }
        let (_, from_index, epoch) = motion_params;
        let from = from_index as f32 / segment_count;
        let to = selected_index as f32 / segment_count;
        let indicator = div()
            .absolute()
            .top_0()
            .bottom_0()
            .flex()
            .items_center()
            .child(
                div()
                    .mx(px(1.0))
                    .w_full()
                    .h(px(28.0))
                    .rounded(cx.theme().radius_lg - px(3.0))
                    .bg(cx.theme().tokens.background)
                    .shadow_sm(),
            )
            .with_animation(
                ElementId::NamedInteger(animation_key, epoch),
                Animation::new(Duration::from_millis(200)).with_easing(ease_in_out),
                move |indicator, delta| {
                    indicator
                        .left(relative(from + (to - from) * delta))
                        .w(relative(1.0 / segment_count))
                },
            );

        div()
            .id(self.id)
            .h(px(36.0))
            .px(px(4.0))
            .rounded(cx.theme().radius_lg)
            .bg(cx.theme().tokens.tab_bar_segmented)
            .refine_style(&self.style)
            .child(
                div()
                    .relative()
                    .size_full()
                    .flex()
                    .items_center()
                    .child(indicator)
                    .children(self.labels.into_iter().enumerate().map(|(index, label)| {
                        let selected = index == selected_index;
                        Button::new(index)
                            .ghost()
                            .large()
                            .flex_1()
                            .h(px(28.0))
                            .rounded(cx.theme().radius_lg - px(3.0))
                            .bg(cx.theme().transparent)
                            .text_color(if selected {
                                cx.theme().tab_active_foreground
                            } else {
                                cx.theme().tab_foreground
                            })
                            .selected(selected)
                            .toggled(selected)
                            .label(label)
                            .when_some(on_click.clone(), move |button, handler| {
                                button.on_click(move |_, window, cx| handler(&index, window, cx))
                            })
                    })),
            )
    }
}
